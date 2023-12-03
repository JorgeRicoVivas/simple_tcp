use std::io::{ErrorKind, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::ops::{Deref, DerefMut};

use fixed_index_vec::fixed_index_vec::FixedIndexVec;

use crate::{Endmark, ENDMARK};

pub struct SimpleServer<ServerData, ClientData> {
    server_socket: TcpListener,
    clients: FixedIndexVec<Client<ClientData>>,
    data: ServerData,
    on_request_accept: fn(&Self, &SocketAddr, &usize) -> Option<ClientData>,
    on_accept: fn(&Self, &usize),
    on_get_message: fn(&mut Self, &usize, &str),
    on_close: fn(&mut Self),
    endmark: Endmark,
    is_blocking: bool,

}

impl<ServerData, ClientData> SimpleServer<ServerData, ClientData> {
    pub fn new(listener: TcpListener, server_data: ServerData, on_accept_clients: fn(&Self, &SocketAddr, &usize) -> Option<ClientData>) -> SimpleServer<ServerData, ClientData> {
        let is_blocking = listener.set_nonblocking(true).is_err();
        Self {
            server_socket: listener,
            clients: FixedIndexVec::new(),
            data: server_data,
            on_request_accept: on_accept_clients,
            on_accept: |_, _| {},
            on_get_message: |_, _, _| {},
            on_close: |_| {},
            endmark: ENDMARK,
            is_blocking,
        }
    }

    pub fn on_accept(&mut self, on_accept: fn(&Self, &usize)) {
        self.on_accept = on_accept;
    }

    pub fn on_request_accept(&mut self, on_request_accept: fn(&Self, &SocketAddr, &usize) -> Option<ClientData>) {
        self.on_request_accept = on_request_accept;
    }

    pub fn on_get_message(&mut self, on_get_message: fn(&mut Self, &usize, &str)) {
        self.on_get_message = on_get_message;
    }

    pub fn on_close(&mut self, on_close: fn(&mut Self)) {
        self.on_close = on_close;
    }

    pub fn accept(&mut self) -> Option<()> {
        let (client_stream, client_socket) = self.server_socket.accept().ok()?;
        self.accept_client(client_stream, client_socket);
        Some(())
    }

    pub fn accept_incoming(&mut self) {
        self.server_socket.incoming()
            .filter(Result::is_ok)
            .map(Result::unwrap)
            .collect::<Vec<_>>().into_iter()
            .for_each(|client_stream| {
                let client_address = client_stream.peer_addr();
                if client_address.is_err() { return; }
                self.accept_client(client_stream, client_address.unwrap());
            });
    }

    fn accept_client(&mut self, stream: TcpStream, socket: SocketAddr) {
        let is_blocking_read = stream.set_nonblocking(true).is_err();
        let id = self.clients.reserve_pos();
        let client_data = (self.on_request_accept)(self, &socket, &id);
        if client_data.is_none() {
            self.clients.remove_reserved_pos(id);
            return;
        }
        let client_data = client_data.unwrap();
        let client = Client { id, stream, socket, message_buffer: String::new(), is_blocking_read, data: client_data };
        (self.on_accept)(self, &id);
        self.clients.push_reserved(client.id, client);
    }

    pub fn read_clients(&mut self, skip_blocking_clients: bool) -> usize {
        let mut total_read_bytes: usize = 0;
        let clients_len = self.clients.len();
        let mut client_index = 0;
        while client_index < clients_len {
            let client = self.clients.get_mut(client_index);
            if client.is_none() || (skip_blocking_clients && client.as_ref().unwrap().is_blocking_read && self.is_blocking) {
                client_index += 1;
                continue;
            }
            let client = client.unwrap();
            let mut stream_read = [0; 1024];
            let result = client.stream.read(&mut stream_read);
            match result {
                Ok(read_bytes) => {
                    total_read_bytes = total_read_bytes.checked_add(read_bytes).unwrap_or_else(|| usize::MAX);
                    let client_suddenly_disconnected = read_bytes == 0;
                    if client_suddenly_disconnected {
                        //The client has discconected without notifying it's connection's end,
                        //this happens when its program was closed forcedly
                        self.remove_client(client_index);
                        println!("Disconnected");
                        continue;
                    }
                    match String::from_utf16(&stream_read.map(|character| character as u16)) {
                        Ok(received_string) => {
                            self.read_clients_input(client_index, &received_string[0..read_bytes]);
                        }
                        Err(_error) => {
                            //Client data is unparseable, making this connection a wrong one
                            self.remove_client(client_index);
                        }
                    }
                }
                Err(error) => {
                    match error.kind() {
                        ErrorKind::WouldBlock => {}
                        ErrorKind::ConnectionReset => {
                            self.remove_client(client_index);
                            println!("Disconnected by reset");
                            continue;
                        }
                        _ => println!("Error is {:?}", error.kind()),
                    };
                    client_index += 1;
                }
            }
        }
        total_read_bytes
    }

    fn read_clients_input(&mut self, client_index: usize, real_received_string: &str) {
        let client = self.clients.get_mut(client_index).unwrap();
        let message = real_received_string;
        let input = &mut client.message_buffer;
        let previous_input_len = input.len();
        input.extend(message.chars());
        let end_bound = crate::message_processing::find_message_end_bound_utf16(&input, input.len(), false,
                                                                                previous_input_len.checked_sub(self.endmark.string.len() + 1).unwrap_or(0), &self.endmark);
        if end_bound.is_none() { return; }
        let end_bound = end_bound.unwrap();
        let mut messages = crate::message_processing::substring_utf16(input, 0, end_bound + self.endmark.string.len());

        let buffer = crate::message_processing::substring_utf16(input, end_bound + self.endmark.string.len(), input.len());
        client.message_buffer = buffer;

        crate::message_processing::find_and_process_messages(&mut messages, 0, &self.endmark.clone(), |message, keep_checking| {
            (self.on_get_message)(self, &client_index, message);
            if !self.clients.contains_index(client_index) {
                *keep_checking = false;
            }
        });
    }

    pub fn read_clients_to_end(&mut self) -> usize {
        let mut total_read_bytes = 0;
        loop {
            match self.read_clients(true) {
                0 => return total_read_bytes,
                read_bytes => total_read_bytes = total_read_bytes.checked_add(read_bytes).unwrap_or_else(|| usize::MAX)
            }
        }
    }

    pub fn get_client(&self, client_id: usize) -> Option<&Client<ClientData>> {
        self.clients.get(client_id)
    }

    pub fn get_client_mut(&mut self, client_id: usize) -> Option<&mut Client<ClientData>> {
        self.clients.get_mut(client_id)
    }

    pub fn clients_len(&self) -> usize {
        self.clients.len()
    }

    pub fn remove_client(&mut self, client_id: usize) -> Option<Client<ClientData>> {
        self.clients.remove(client_id)
    }

    pub fn send_message_to_client(&mut self, client: usize, message: &str) -> Option<std::io::Result<usize>> {
        let client = self.clients.get_mut(client);
        if client.is_none() { return None; }
        let mut message = message.replace(self.endmark.string, self.endmark.escape);
        message.extend(self.endmark.string.chars());
        Some(client.unwrap().stream.write(message.as_bytes()))
    }

    pub fn send_message_to_clients(&mut self, clients: &[usize], message: &str) -> Vec<Option<std::io::Result<usize>>> {
        let mut message = message.replace(self.endmark.string, self.endmark.escape);
        message.extend(self.endmark.string.chars());
        clients.into_iter().map(|&client| {
            let client = self.clients.get_mut(client);
            if client.is_none() { return None; }
            Some(client.unwrap().stream.write(message.as_bytes()))
        }).collect::<Vec<_>>()
    }

    pub fn send_message_to_all_clients(&mut self, message: &str) -> Vec<std::io::Result<usize>> {
        let mut message = message.replace(self.endmark.string, self.endmark.escape);
        message.extend(self.endmark.string.chars());
        self.clients.iter_mut().map(|client| {
            client.stream.write(message.as_bytes())
        }).collect::<Vec<_>>()
    }
    pub fn data(&self) -> &ServerData {
        &self.data
    }

    pub fn data_mut(&mut self) -> &mut ServerData {
        &mut self.data
    }
}

impl<ServerData, ClientData> Deref for SimpleServer<ServerData, ClientData> {
    type Target = ServerData;

    fn deref(&self) -> &Self::Target {
        self.data()
    }
}

impl<ServerData, ClientData> DerefMut for SimpleServer<ServerData, ClientData> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.data_mut()
    }
}

impl<ServerData, ClientData> Drop for SimpleServer<ServerData, ClientData> {
    fn drop(&mut self) {
        (self.on_close)(self);
        self.clients.iter_mut().for_each(|client| {
            let _ = client.stream.shutdown(Shutdown::Both);
        });
    }
}

pub struct Client<ClientData> {
    id: usize,
    stream: TcpStream,
    socket: SocketAddr,
    message_buffer: String,
    is_blocking_read: bool,
    data: ClientData,
}

impl<ClientData> Client<ClientData> {
    pub fn id(&self) -> usize {
        self.id
    }

    pub fn address(&self) -> &SocketAddr {
        &self.socket
    }

    pub fn data(&self) -> &ClientData {
        &self.data
    }

    pub fn data_mut(&mut self) -> &mut ClientData {
        &mut self.data
    }
}

impl<ClientData> Deref for Client<ClientData> {
    type Target = ClientData;

    fn deref(&self) -> &Self::Target {
        self.data()
    }
}

impl<ClientData> DerefMut for Client<ClientData> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.data_mut()
    }
}
