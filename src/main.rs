use std::io::{Read, Write};
use std::net::TcpListener;
use std::ops::Deref;
use std::thread::sleep;
use std::time::Duration;
use server::SimpleServer;

#[derive(Clone)]
struct Endmark {
    string: &'static str,
    escape: &'static str,
}

const ENDMARK: Endmark = Endmark {
    string: "EOF",
    escape: "\\EOF",
};

fn main() {
    let listener = TcpListener::bind("127.0.0.1:5050").unwrap();
    println!("{}", listener.set_nonblocking(true).is_ok());
    let mut server = SimpleServer::<u16>::new(listener, |_, _, _| { None });
    server.on_accept(|server,address, client_id| Some(*client_id as u16));
    server.on_get_message(|server, client_id , message| {
        println!("Got message from {}, contents are:", client_id);
        println!("{message}");
        let client_address = server.get_client(*client_id).unwrap().address();
        server.send_message_to_all_clients(&*format!("Client from {client_address} sent: {message}"));
    });
    loop {
        println!("---New cycle---");
        println!("-Accepting clients-");
        server.accept();
        println!("-Reading clients-");
        server.read_clients();
        println!("---End of cycle---");
        sleep(Duration::from_secs(1));
    }
}

pub mod server;
pub(crate) mod message_processing;