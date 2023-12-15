use std::io::{ErrorKind, Read, Write};
use std::mem;
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::ops::{Deref, DerefMut};
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use fixed_index_vec::fixed_index_vec::FixedIndexVec;

use crate::message_processing::{DEFAULT_ENDMARK, Endmark};
use crate::server::{AcceptError, Server, ServerAcceptError};

pub mod builder;

pub struct SimpleServer<ServerData, ClientData> {
    pub(crate) inner: RwLock<InnerSimpleServer<ServerData, ClientData>>,
}

impl<ServerData, ClientData> Server for SimpleServer<ServerData, ClientData> {
    fn accept(&self) -> Result<usize, AcceptError> {
        InnerSimpleServer::<ServerData, ClientData>::accept(&self.inner)
    }

    fn accept_incoming_not_blocking(&self) -> Result<Vec<Result<usize, AcceptError>>, ServerAcceptError> {
        InnerSimpleServer::<ServerData, ClientData>::accept_incoming_not_blocking(&self.inner)
    }

    fn accept_incoming(&self) -> Vec<Result<usize, AcceptError>> {
        InnerSimpleServer::<ServerData, ClientData>::accept_incoming(&self.inner)
    }

    fn read_client(&self, client_index: usize) -> Option<usize> {
        InnerSimpleServer::<ServerData, ClientData>::read_client(&self.inner, client_index)
    }

    fn read_clients(&self, skip_blocking_clients: bool) -> usize {
        InnerSimpleServer::<ServerData, ClientData>::read_clients(&self.inner, skip_blocking_clients)
    }

    fn clients_len(&self) -> usize {
        self.inner.read().unwrap().clients().len()
    }

    fn send_message_to_client(&self, client: usize, message: &str) -> Option<std::io::Result<usize>> {
        self.inner.read().unwrap().send_message_to_client(client, message)
    }

    fn send_message_to_clients(&self, clients: &[usize], message: &str) -> Vec<Option<std::io::Result<usize>>> {
        self.inner.read().unwrap().send_message_to_clients(clients, message)
    }
}


#[derive(Debug)]
pub struct InnerSimpleServer<ServerData, ClientData> {
    server_socket: TcpListener,
    clients: RwLock<FixedIndexVec<Client<ClientData>>>,
    data: ServerData,
    filter_request_accept: fn(&RwLock<Self>, &SocketAddr, &usize) -> Option<ClientData>,
    on_accept: fn(&RwLock<Self>, &usize),
    on_get_message: fn(&RwLock<Self>, &usize, &str),
    on_close: fn(&mut Self),
    endmark: Endmark,
    is_blocking: bool,
}

impl<ServerData, ClientData> InnerSimpleServer<ServerData, ClientData> {
    /// Generates a new Server listening incoming communications through the indicated listener.
    /// <br>
    /// * `listener`: Listener which accepts new clients/connections.
    /// <br>
    /// <br>
    /// * `server_data`: Common data held by this server, this allows you to bundle information your
    /// server requires to function inside of the same structure, to avoid requiring you the need of
    /// implementing your own mechanisms.
    /// <br>
    /// <br>
    /// * `filter_request_accept`: Function pointer to specifying how to filter an incoming
    /// connection based on a client's [SocketAddr], to confirm a client's connection, this function
    /// should return [Some] where the contents of [Some] are the initial data of a client
    /// ([ClientData]).<br>For further details, see [InnerSimpleServer::filter_request_accept].
    /// <br>
    /// <br>
    /// # Simple example:
    ///
    /// This creates a server where every client is initialized with their client index upon
    /// creation.
    ///
    /// ```rust
    /// use std::net::TcpListener;
    /// use simple_tcp::simple_server::InnerSimpleServer;
    ///
    /// let mut server = InnerSimpleServer::new(
    ///     TcpListener::bind("192.168.1.170:8080").unwrap(),
    ///     // This server does not hold data
    ///     (),
    ///     |server, client_socket, client_index|{
    ///         // We initialize incoming clients by giving them their index, we could have written
    ///         // Some(()) in order to not store information about them
    ///         Some(client_index)
    ///     });
    /// ```
    /// <br>
    /// <br>
    ///
    /// # Complex example:
    ///
    /// This creates a server with a blacklist and a name, where each client is initialized with its
    /// index and a 'random' seed.
    ///
    /// ```rust
    /// use std::collections::HashSet;
    /// use std::net::{IpAddr, TcpListener};
    /// use std::str::FromStr;
    /// use simple_tcp::simple_server::InnerSimpleServer;
    ///
    /// struct ServerInfo{
    ///     server_name: String,
    ///     black_list: HashSet<IpAddr>,
    /// }
    ///
    /// struct ClientInfo{
    ///     client_index: usize,
    ///     seed: u128,
    /// }
    ///
    /// let mut server = InnerSimpleServer::new(
    ///     TcpListener::bind("192.168.1.170:8080").unwrap(),
    ///     // Gives initial data to the server
    ///     ServerInfo{
    ///         server_name: "MySimpleServer".to_string(),
    ///         // Blacklists IPs
    ///         black_list: HashSet::from(["192.168.1.100", "192.168.1.120", "192.168.1.140"]
    ///             .map(|ip| IpAddr::from_str(ip).unwrap())),
    ///     },
    ///     |server, client_socket, client_index|{
    ///         // If client's IP belongs to one of the blacklisted ones, the connection gets cancelled
    ///         if server.read().unwrap().black_list.contains(&client_socket.ip()){
    ///             return None;
    ///         }
    ///         // The client it's whitelisted, so we initialize it's data in order to accept it
    ///         Some(ClientInfo{client_index: *client_index, seed: 18274})
    ///     });
    ///
    /// // Blacklists another IP
    /// server.black_list.insert(IpAddr::from_str("192.168.1.160").unwrap());
    /// ```
    pub fn new(listener: TcpListener, server_data: ServerData, filter_request_accept: fn(&RwLock<InnerSimpleServer<ServerData, ClientData>>, &SocketAddr, &usize) -> Option<ClientData>) -> InnerSimpleServer<ServerData, ClientData> {
        let is_blocking = listener.set_nonblocking(true).is_err();
        Self {
            server_socket: listener,
            clients: RwLock::new(FixedIndexVec::new()),
            data: server_data,
            filter_request_accept,
            on_accept: |_, _| {},
            on_get_message: |_, _, _| {},
            on_close: |_| {},
            endmark: DEFAULT_ENDMARK,
            is_blocking,
        }
    }

    /// Specifies how to filter an incoming connection based on a client's [SocketAddr], to confirm
    /// a client's connection, this function should return [Some] where the contents of [Some] are
    /// the initial data of a client ([ClientData]).
    /// <br>
    /// <br>
    /// ```no_run rust
    /// let mut server : simple_tcp::simple_server::InnerSimpleServer<(),String> = ...;
    ///
    /// server.filter_request_accept(|server, client_socket, client_index|{
    ///     // Only accepts using IPv4 whose IP starts with '192'
    ///     if !client_socket.is_ipv4() && !client_socket.to_string().starts_with("192"){
    ///         // Rejects connection from an IP that is not v4 or not in 192.x.y.z
    ///         return None;
    ///     }
    ///     // Accepts this client
    ///     Some("My client from 192.x.y.z".to_string())
    /// });
    /// ```
    pub fn filter_request_accept(&mut self, on_request_accept: fn(&RwLock<Self>, &SocketAddr, &usize) -> Option<ClientData>) {
        self.filter_request_accept = on_request_accept;
    }

    /// Indicates an action to take once a client is accepted (This is after filter_request is
    /// executed, meaning Clients already have been included to the server and their data is valid).
    /// <br>
    /// <br>
    /// This is commonly used to initialize communications to the client.
    /// <br>
    /// <br>
    ///
    /// ```no_run rust
    /// let mut server : simple_tcp::simple_server::InnerSimpleServer<(),String> = ...;
    ///
    /// server.on_accept(|server, accepted_client_index|{
    ///     // Gets the string of the client (This is because the [ClientData] type is 'String',
    ///     // where [ClientData] derefs to said String)
    ///     let client_string = &*server.get_client(*accepted_client_index).unwrap();
    ///     let salutation_message = format!("Welcome to my app client {}!", client_string);
    ///     // Sends a salutation message to the client
    ///     server.send_message_to_client(*accepted_client_index, &*salutation_message);
    /// });
    /// ```
    pub fn on_accept(&mut self, on_accept: fn(&RwLock<Self>, &usize)) {
        self.on_accept = on_accept;
    }

    pub fn on_get_message(&mut self, on_get_message: fn(&RwLock<Self>, &usize, &str)) {
        self.on_get_message = on_get_message;
    }

    pub fn on_close(&mut self, on_close: fn(&mut Self)) {
        self.on_close = on_close;
    }

    pub fn accept(locked_self: &RwLock<Self>) -> Result<usize, AcceptError> {
        let accept = locked_self.read().unwrap().server_socket.accept();
        if accept.is_err() {
            return Err(AcceptError::IOError(accept.err().unwrap()));
        }
        let (client_stream, client_socket) = accept.unwrap();
        Self::join_filter_client(locked_self, client_stream, client_socket)
    }

    pub fn accept_incoming_not_blocking(locked_self: &RwLock<Self>) -> Result<Vec<Result<usize, AcceptError>>, ServerAcceptError> {
        if locked_self.read().unwrap().is_blocking { return Err(ServerAcceptError::IsBlocking); }
        let mut clients = Vec::new();
        loop {
            let incoming = locked_self.read().unwrap().server_socket.accept().ok();
            match incoming {
                None => return Ok(clients),
                Some((client_stream, client_socket)) => {
                    clients.push(Self::join_filter_client(locked_self, client_stream, client_socket));
                }
            }
        }
    }

    fn accept_incoming(locked_self: &RwLock<Self>) -> Vec<Result<usize, AcceptError>> {
        locked_self.read().unwrap().server_socket.incoming()
            .filter(Result::is_ok)
            .map(Result::unwrap)
            .collect::<Vec<_>>()
            .into_iter()
            .map(|client_stream| {
                let client_address = client_stream.peer_addr();
                if client_address.is_err() { return Err(AcceptError::CouldNotGetSocket(client_stream)); }
                Self::join_filter_client(locked_self, client_stream, client_address.unwrap())
            })
            .collect()
    }

    fn join_filter_client(locked_self: &RwLock<Self>, stream: TcpStream, socket: SocketAddr) -> Result<usize, AcceptError> {
        let is_blocking_read = stream.set_nonblocking(true).is_err();

        let id = (&mut *locked_self.write().unwrap()).clients.write().unwrap().reserve_pos();
        let client_data = (locked_self.read().unwrap().filter_request_accept)(locked_self, &socket, &id);
        if client_data.is_none() {
            locked_self.write().unwrap().clients.write().unwrap().remove_reserved_pos(id);
            return Err(AcceptError::DeniedSocket(socket));
        }
        let client_data = client_data.unwrap();
        let client = Client { id, stream: RwLock::new(stream), socket, message_buffer: String::new(), is_blocking_read, should_remove: false, data: client_data };
        locked_self.write().unwrap().clients.write().unwrap().push_reserved(client.id, client);
        (locked_self.read().unwrap().on_accept)(locked_self, &id);
        Ok(id)
    }

    pub fn read_client(locked_self: &RwLock<Self>, client_index: usize) -> Option<usize> {
        if !locked_self.read().unwrap().clients().contains_index(client_index) {
            return None;
        }
        if locked_self.read().unwrap().clients().get(client_index).unwrap().should_remove {
            return None;
        }
        let mut stream_read = [0; 1024];
        let mut total_read_bytes = 0_usize;
        loop {
            if locked_self.read().unwrap().clients().get(client_index).unwrap().should_remove {
                return None;
            }
            let read = (&*locked_self.read().unwrap()).clients().get(client_index).unwrap().stream.write().unwrap().read(&mut stream_read);
            match read {
                Ok(read_bytes) => {
                    total_read_bytes = total_read_bytes.checked_add(read_bytes).unwrap_or_else(|| usize::MAX);
                    let client_suddenly_disconnected = read_bytes == 0;
                    if client_suddenly_disconnected {
                        //The client has disconnected without notifying it's connection's end,
                        //this happens when its program was closed forcedly
                        (&*locked_self.read().unwrap()).clients.write().unwrap().get_mut(client_index).unwrap().should_remove = true;
                        continue;
                    }
                    match String::from_utf16(&stream_read.map(|character| character as u16)) {
                        Ok(received_string) => {
                            Self::read_client_input(locked_self, client_index, &received_string[0..read_bytes])
                        }
                        Err(_error) => {
                            //Client data is unparseable, making this connection a wrong one
                            (&*locked_self.read().unwrap()).clients.write().unwrap().get_mut(client_index).unwrap().should_remove = true;
                        }
                    }
                }
                Err(error) => {
                    match error.kind() {
                        ErrorKind::WouldBlock => {}
                        ErrorKind::ConnectionReset => {
                            (&*locked_self.read().unwrap()).clients_mut().remove(client_index);
                            continue;
                        }
                        _ => {}
                    };
                    break;
                }
            }
        }
        Some(total_read_bytes)
    }

    pub fn read_clients(locked_self: &RwLock<Self>, skip_blocking_clients: bool) -> usize {
        if locked_self.read().unwrap().is_blocking && skip_blocking_clients {
            return 0;
        }
        let mut total_read_bytes: usize = 0;
        let clients_len = locked_self.read().unwrap().clients.read().unwrap().len();
        let mut client_index = 0;
        while client_index < clients_len {
            if !locked_self.read().unwrap().clients.read().unwrap().contains_index(client_index)
                || locked_self.read().unwrap().clients().get(client_index).unwrap().is_blocking_read && skip_blocking_clients {
                client_index += 1;
                continue;
            }
            let read_bytes = Self::read_client(locked_self, client_index).unwrap_or_else(|| 0);
            total_read_bytes = total_read_bytes.checked_add(read_bytes).unwrap_or_else(|| usize::MAX);
            if locked_self.read().unwrap().clients().get(client_index).unwrap().should_remove {
                locked_self.read().unwrap().clients.write().unwrap().remove(client_index);
            } else {
                client_index += 1;
            }
        }
        total_read_bytes
    }

    fn read_client_input(locked_self: &RwLock<Self>, client_index: usize, real_received_string: &str) {
        let ref_self = &*locked_self.read().unwrap();
        let message = real_received_string;
        let mut input = mem::take(&mut ref_self.clients_mut().get_mut(client_index).unwrap().message_buffer);
        let previous_input_len = input.len();
        input.extend(message.chars());
        let end_bound = crate::message_processing::find_message_end_bound_utf16(&input, input.len(), false,
                                                                                previous_input_len.checked_sub(ref_self.endmark.string().len() + 1).unwrap_or(0), &ref_self.endmark);
        if end_bound.is_none() { return; }
        let end_bound = end_bound.unwrap();
        let mut messages = crate::message_processing::substring_utf16(&input, 0, end_bound + ref_self.endmark.string().len());

        let buffer = crate::message_processing::substring_utf16(&input, end_bound + ref_self.endmark.string().len(), input.len());
        ref_self.clients_mut().get_mut(client_index).unwrap().message_buffer = buffer;

        crate::message_processing::find_and_process_messages(&mut messages, 0, &ref_self.endmark.clone(), |message, keep_checking| {
            (ref_self.on_get_message)(locked_self, &client_index, message);
            *keep_checking = ref_self.clients.read().unwrap().get(client_index).map(|client| !client.should_remove).unwrap_or_else(|| false);
        });
    }

    pub fn clients_lock(&self) -> &RwLock<FixedIndexVec<Client<ClientData>>> {
        &self.clients
    }

    pub fn clients(&self) -> RwLockReadGuard<'_, FixedIndexVec<Client<ClientData>>> {
        self.clients.read().unwrap()
    }

    pub fn clients_mut(&self) -> RwLockWriteGuard<'_, FixedIndexVec<Client<ClientData>>> {
        self.clients.write().unwrap()
    }

    pub fn send_message_to_client(&self, client: usize, message: &str) -> Option<std::io::Result<usize>> {
        if !self.clients.read().unwrap().contains_index(client) {
            return None;
        }
        let message = self.endmark.prepare_message(message);
        Some(self.clients.read().unwrap().get(client).unwrap().stream.write().unwrap().write(message.as_bytes()))
    }

    pub fn send_message_to_clients(&self, clients: &[usize], message: &str) -> Vec<Option<std::io::Result<usize>>> {
        let message = self.endmark.prepare_message(message);
        clients.iter().map(|&client| {
            self.clients.read().unwrap().get(client).map(|client|
                client.stream.write().unwrap().write(message.as_bytes()))
        }).collect::<Vec<_>>()
    }
    pub fn data(&self) -> &ServerData {
        &self.data
    }

    pub fn data_mut(&mut self) -> &mut ServerData {
        &mut self.data
    }

    pub fn message_endmark(&self) -> &Endmark {
        &self.endmark
    }

    pub fn set_nonblocking(&mut self, non_blocking: bool) -> Result<(), ()> {
        let result = self.server_socket.set_nonblocking(non_blocking);
        if result.is_err() { return Err(()); }
        self.is_blocking = !non_blocking;
        Ok(())
    }
}

impl<ServerData, ClientData> Deref for InnerSimpleServer<ServerData, ClientData> {
    type Target = ServerData;

    fn deref(&self) -> &Self::Target {
        self.data()
    }
}

impl<ServerData, ClientData> DerefMut for InnerSimpleServer<ServerData, ClientData> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.data_mut()
    }
}

impl<ServerData, ClientData> Drop for InnerSimpleServer<ServerData, ClientData> {
    fn drop(&mut self) {
        (self.on_close)(self);
        self.clients.write().unwrap().iter_mut().for_each(|client| {
            let _ = client.stream.read().unwrap().shutdown(Shutdown::Both);
        });
    }
}

#[derive(Debug)]
pub struct Client<ClientData> {
    id: usize,
    stream: RwLock<TcpStream>,
    socket: SocketAddr,
    message_buffer: String,
    is_blocking_read: bool,
    should_remove: bool,
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

    pub fn mark_to_remove(&mut self) -> bool {
        if self.should_remove {
            return false;
        }
        self.should_remove = true;
        true
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
