
use std::net::{SocketAddr};

pub enum AcceptError {
    IOError(std::io::Error),
    CouldNotGetSocket(std::net::TcpStream),
    DeniedSocket(SocketAddr),
}

pub enum ServerAcceptError {
    IsBlocking
}

#[derive(Default)]
pub struct ClientRead{
    pub(crate) bytes_read: usize,
    pub(crate) messages: Vec<String>,
}

impl ClientRead {
    pub fn bytes_read(&self) -> usize {
        self.bytes_read
    }

    pub fn messages(&self) -> &Vec<String> {
        &self.messages
    }

    pub fn into_messages(self) -> Vec<String> {
        self.messages
    }

}


pub trait Server {

    /// Accepts one client, giving back the id of said client in case of being accepted.
    fn accept_no_context(&self) -> Result<usize, AcceptError>;

    /// Accepts every incoming client while not blocking.
    ///
    /// It should return [Err] if the server is blocking, see [TcpListener::set_nonblocking].
    fn accept_incoming_not_blocking(&self) -> Result<Vec<Result<usize, AcceptError>>, ServerAcceptError>;

    /// Accepts every incoming client while blocking until there is at least one client.
    fn accept_incoming_no_context(&self) -> Vec<Result<usize, AcceptError>>;

    /// Reads one clients, giving a [ClientRead] containing amount of bytes it JUST read from this
    /// client, and messages gotten after said read, this can accumulate messages that weren't
    /// complete on previous reads.
    ///
    /// Returns [None] whether this client doesn't exists or has been marked to be removed (Note you
    /// can manually set clients to be removed, but other actions can also mark clients for removal,
    /// such as a disconnection, or a wrong written character inside a message).
    fn read_client(&self, client_index: usize) -> Option<ClientRead>;

    /// Reads all clients as in [Self::read_client], returning a Vector containing the index of
    /// every client that was read along it's messages
    fn read_clients(&self, skip_blocking_clients: bool) -> Vec<(usize, ClientRead)>;

    /// Reads one clients, triggering a custom action on every received message
    fn read_client_no_context(&self, client_index: usize);

    /// Reads all clients, triggering a custom action on every received message per client
    fn read_clients_no_context(&self, skip_blocking_clients: bool){
        for client_index in 0..self.clients_len(){
            self.read_client_no_context(client_index);
        }
    }

    /// Amount of clients.
    fn clients_len(&self) -> usize;

    /// Sends a message to a client.
    ///
    /// Returns [None] if the index does not match to an existing client.
    fn send_message_to_client(&self, client: usize, message: &str) -> Option<std::io::Result<usize>>;

    /// Sends a message to every indicated client.
    ///
    /// Returns [None] for each index not matching to an existing client.
    fn send_message_to_clients(&self, clients: &[usize], message: &str) -> Vec<Option<std::io::Result<usize>>> {
        clients.into_iter().map(|&client_index| {
            self.send_message_to_client(client_index, message)
        }).collect()
    }

    /// Sends a message to all clients, returning the amount of bytes written on each one of them.
    ///
    /// Returns [None] for each index not matching to an existing client.
    fn send_message_to_all_clients(&self, message: &str) -> Vec<Option<std::io::Result<usize>>> {
        (0..self.clients_len()).into_iter().map(|client_index| {
            self.send_message_to_client(client_index, message)
        }).collect()
    }
}