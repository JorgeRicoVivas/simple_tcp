use std::net::{SocketAddr, TcpListener};
use crate::message_processing::Endmark;
use crate::simple_server::{InnerSimpleServer, SimpleServer};
use crate::unchecked_read_write_lock::UncheckedRwLock;

pub struct SimpleServerBuilder<ServerData, ClientData> {
    server_socket: TcpListener,
    data: ServerData,
    filter_request_accept: fn(&SimpleServer<ServerData, ClientData>, &SocketAddr, usize) -> Option<ClientData>,
    on_accept: Option<fn(&SimpleServer<ServerData, ClientData>, usize)>,
    on_get_message: Option<fn(&SimpleServer<ServerData, ClientData>, usize, String)>,
    on_close: Option<fn(&mut SimpleServer<ServerData, ClientData>)>,
    endmark: Option<Endmark>,
}

impl<ServerData, ClientData> SimpleServerBuilder<ServerData, ClientData> {
    pub fn new(listener: TcpListener, server_data: ServerData, on_request_accept: fn(&SimpleServer<ServerData, ClientData>, &SocketAddr, usize) -> Option<ClientData>)
               -> SimpleServerBuilder<ServerData, ClientData> {
        Self {
            server_socket: listener,
            data: server_data,
            filter_request_accept: on_request_accept,
            on_accept: None,
            on_get_message: None,
            on_close: None,
            endmark: None,
        }
    }

    pub fn on_accept(mut self, on_accept: fn(&SimpleServer<ServerData, ClientData>, usize)) -> SimpleServerBuilder<ServerData, ClientData> {
        self.on_accept = Some(on_accept);
        self
    }

    pub fn on_get_message(mut self, on_get_message: fn(&SimpleServer<ServerData, ClientData>, usize, String)) -> SimpleServerBuilder<ServerData, ClientData> {
        self.on_get_message = Some(on_get_message);
        self
    }

    pub fn on_close(mut self, on_close: fn(&mut SimpleServer<ServerData, ClientData>)) -> SimpleServerBuilder<ServerData, ClientData> {
        self.on_close = Some(on_close);
        self
    }

    pub fn endmark(mut self, endmark: Endmark) -> SimpleServerBuilder<ServerData, ClientData> {
        self.endmark = Some(endmark);
        self
    }

    pub fn build(self) -> SimpleServer<ServerData, ClientData> {
        let mut server = SimpleServer::new(InnerSimpleServer::new(self.server_socket, self.data));
        server.filter_request_accept(self.filter_request_accept);
        if self.on_accept.is_some() {
            server.on_accept(self.on_accept.unwrap());
        }
        if self.on_get_message.is_some() {
            server.on_get_message(self.on_get_message.unwrap());
        }
        if self.on_close.is_some() {
            server.on_close(self.on_close.unwrap());
        }
        if self.endmark.is_some() {
            server.inner.write().endmark = self.endmark.unwrap();
        }
        server
    }
}