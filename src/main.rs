use actix::{Actor, StreamHandler};
use actix_web::{web, App, HttpRequest, HttpResponse, HttpServer};
use actix_web_actors::ws;

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

/// Define http actor
struct WsZmqActor {
    zmq_publisher: zmq::Socket,
    _zmq_ctx: zmq::Context,
}

impl WsZmqActor {
    pub fn new(endpoint: &str) -> Result<WsZmqActor, CliError> {
        let ctx = zmq::Context::new();
        let publisher = ctx.socket(zmq::PUB)?;
        publisher.connect(endpoint)?;

        Ok(WsZmqActor {
            zmq_publisher: publisher,
            _zmq_ctx: ctx,
        })
    }
}

impl Actor for WsZmqActor {
    type Context = ws::WebsocketContext<Self>;
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
                println!("Text: {:#?}", text);
                self.zmq_publisher.send(text.as_bytes(), 0).unwrap();
                ctx.text(text);
            }
            _ => (),
        }
    }
}

async fn index(req: HttpRequest, stream: web::Payload) -> Result<HttpResponse, actix_web::Error> {
    let connector = match WsZmqActor::new("tcp://ubuntu20:5555") {
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
    // Run a ZMQ subscriber on one thread
    std::thread::spawn(|| {
        let ctx = zmq::Context::new();
        let subscriber = ctx.socket(zmq::SUB).unwrap();
        subscriber.connect("tcp://ubuntu20:5556").unwrap();
        subscriber.set_subscribe(&vec![]).unwrap();

        println!("Subscribed!");

        loop {
            let message= subscriber.recv_string(0).unwrap().unwrap();
            println!("{}", message);
        }
    });

    // Run a WS server on one thread
    HttpServer::new(|| App::new().route("/ws/", web::get().to(index)))
        .bind("127.0.0.1:8088")?
        .run()
        .await
}
