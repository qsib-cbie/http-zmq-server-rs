use actix::{Actor, StreamHandler};
use actix_web::{web, App, HttpRequest, HttpResponse, HttpServer};
use actix_web_actors::ws;
use actix_web_actors::ws::WebsocketContext;
use std::str;

enum CliError {
    ZmqError(zmq::Error),
}

impl std::convert::From<zmq::Error> for CliError {
    fn from(err: zmq::Error) -> Self {
        CliError::ZmqError(err)
    }
}

impl std::convert::From<CliError> for actix_web::Error {
    fn from(_: CliError) -> Self {
        actix_web::Error::from(std::io::Error::new(std::io::ErrorKind::Other, "CliError"))
    }
}

/// Define http actor with connected zmq socket (and context)
struct WsZmqActor {
    zmq_req_dealer: zmq::Socket,
    _zmq_ctx: zmq::Context,
}

impl WsZmqActor {
    pub fn new(endpoint: &str) -> Result<WsZmqActor, CliError> {
        let ctx = zmq::Context::new();

        let req_dealer= ctx.socket(zmq::DEALER)?;
        req_dealer.connect(endpoint)?;

        Ok(WsZmqActor {
            zmq_req_dealer: req_dealer,
            _zmq_ctx: ctx,
        })
    }

    fn _forward_message(&mut self, request_string: &str) -> Result<(), zmq::Error> {
        // Send the request
        self.zmq_req_dealer.send(&vec![], zmq::SNDMORE)?;        // Simulated REQ: Empty Frame
        self.zmq_req_dealer.send(request_string.as_bytes(), 0)?; // Simulated REQ: Message Content
        Ok(())
    }

    fn _receive_message(&mut self) -> Result<Vec<u8>, zmq::Error> {
        // Send the request
        if self.zmq_req_dealer.poll(zmq::POLLIN, 10000)? == 0 {
            return Ok(vec![]);
        }

        let _ = self.zmq_req_dealer.recv_bytes(0)?;                    // Simulated REQ: Empty Frame
        let msg = self.zmq_req_dealer.recv_bytes(0)?;         // Simulated REQ: Message Content
        return Ok(msg)

    }
}

impl Actor for WsZmqActor {
    type Context = WebsocketContext<Self>;
}

/// Handler for ws::Message message
impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WsZmqActor {
    fn handle(
        &mut self,
        msg: Result<ws::Message, ws::ProtocolError>,
        ctx: &mut Self::Context,
    ) {
        match msg {
            Ok(ws::Message::Ping(msg)) => ctx.pong(&msg),
            Ok(ws::Message::Binary(bin)) => ctx.binary(bin),
            Ok(ws::Message::Text(text)) => {
                // Try find response
                if text == "poll" {
                    // Request is sent, Wait for the Response
                    let resp = match self._receive_message() {
                        Ok(msg) => {
                            msg
                        },
                        Err(err) => {
                            println!("Failed to receive response to foward: {}", err.to_string());
                            return;
                        }
                    };

                    // Send the response if there was one
                    println!("Found response: {}", String::from(str::from_utf8(resp.as_slice()).unwrap_or("!!! E R R O R !!!")));
                    if resp.len() > 0 {
                        let string = String::from_utf8(resp);
                        if string.is_ok() {
                            ctx.text(string.unwrap().as_str());
                        }
                    }
                } else {
                    // Send the message
                    println!("Found Request: {}", text);
                    match self._forward_message(text.as_str()) {
                        Ok(_) => { },
                        Err(err) => {
                            // Swallowing ZMQ forwarding error
                            println!("Failed to forward: {} due to: {}", &text, err);
                            return;
                        }
                    }
                }
            }
            _ => (),
        }
    }
}

async fn index(req: HttpRequest, stream: web::Payload) -> Result<HttpResponse, actix_web::Error> {
    let connector = match WsZmqActor::new("tcp://ubuntu20:6000") {
        Ok(connector) => connector,
        Err(err) => {
            return Err(std::convert::From::from(err));
        }
    };
    let resp = ws::start(connector, &req, stream);
    println!("{:?}", resp);
    resp
}

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    // Run a WS server
    HttpServer::new(|| App::new().route("/ws/", web::get().to(index)))
        .bind("127.0.0.1:8088")?
        .run()
        .await
}
