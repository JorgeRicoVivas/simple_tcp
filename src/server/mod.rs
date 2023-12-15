
use std::net::{SocketAddr};

pub enum AcceptError {
    IOError(std::io::Error),
    CouldNotGetSocket(std::net::TcpStream),
    DeniedSocket(SocketAddr),
}

pub enum ServerAcceptError {
    IsBlocking
}

pub trait Server {

    /// Accepts one client, giving back the id of said client in case of being accepted.
    fn accept(&self) -> Result<usize, AcceptError>;

    /// Accepts every incoming client while not blocking.
    ///
    /// It should return [Err] if the server is blocking, see [TcpListener::set_nonblocking].
    fn accept_incoming_not_blocking(&self) -> Result<Vec<Result<usize, AcceptError>>, ServerAcceptError>;

    /// Accepts every incoming client while blocking until there is at least one client.
    fn accept_incoming(&self) -> Vec<Result<usize, AcceptError>>;

    /// Reads one clients, triggering a custom action per message received.
    ///
    /// Returns the amount of bytes read from this client.
    fn read_client(&self, client_index: usize) -> Option<usize>;

    /// Reads all clients, triggering a custom action per each client and message received.
    ///
    /// Returns the amount of bytes read between all clients.
    fn read_clients(&self, skip_blocking_clients: bool) -> usize;

    /// Reads all clients until there is no new messages.
    ///
    /// Returns the amount of bytes read between all clients and cycles.
    fn read_clients_to_end(&self) -> usize {
        let mut total_read_bytes = 0;
        loop {
            match self.read_clients(true) {
                0 => return total_read_bytes,
                read_bytes => total_read_bytes = total_read_bytes.checked_add(read_bytes).unwrap_or_else(|| usize::MAX)
            }
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