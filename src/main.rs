use actix_web::{web, http, App, HttpResponse, HttpServer, Responder};
use actix_web::post;
use actix_cors::Cors;

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

struct AppState {
    foo: String,
    always_foo: bool,
}

fn try_connect(zmq_ctx: &zmq::Context) -> Result<zmq::Socket, zmq::Error> {
    let zmq_req_dealer = zmq_ctx.socket(zmq::DEALER)?;
    zmq_req_dealer.connect("tcp://ubuntu20:6000")?;
    Ok(zmq_req_dealer)
}

fn try_hit_service(request_string: &[u8]) -> Result<std::vec::Vec<u8>, zmq::Error> {
    // Establish a connection
    let ctx = zmq::Context::new();
    let sckt = try_connect(&ctx)?;

    // Send the request
    sckt.send(&vec![], zmq::SNDMORE)?;        // Simulated REQ: Empty Frame
    sckt.send(request_string, 0)?; // Simulated REQ: Message Content

    if sckt.poll(zmq::POLLIN, 0)? != 0 {
        println!("Sent message with pending input ...");
    }

    // Receive the request
    let mut i = 0;
    loop {
        if sckt.poll(zmq::POLLIN, 1000)? == 0 {
            i += 1;
            if i == 10 {
                return Ok(vec![]);
            }
            println!("Waiting on poll {} ...", i);
        } else {
            break;
        }
    }

    let _ = sckt.recv_bytes(0)?;    // Simulated REQ: Empty Frame
    Ok(sckt.recv_bytes(0)?)         // Simulated REQ: Message Content
}

#[post("/api_index")]
async fn api_index(bytes: web::Bytes, data: web::Data<AppState>) -> impl Responder {
    println!("Request: {:#?}", bytes);

    if data.always_foo {
        println!("Response: {:?}", &data.foo);
        return HttpResponse::Ok().body(&data.foo);
    }

    let msg = match try_hit_service(bytes.as_ref()) {
        Ok(zmq_req_dealer) => {
            zmq_req_dealer
        },
        Err(err) => {
            println!("Failed to connect backend socket: {:#?}", err);
            return HttpResponse::InternalServerError().body("Internal network failure");
        }
    };

    println!("Response: {:?}", String::from_utf8(bytes.to_vec()));
    HttpResponse::Ok().body(msg)
}



#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    // Run an HTTP server
    HttpServer::new(|| App::new()
            .wrap(
                Cors::new() // <- Construct CORS middleware builder
                  .allowed_origin("*")
                  .allowed_origin("http://localhost:3000")
                  .allowed_methods(vec!["GET", "POST"])
                  .allowed_headers(vec![http::header::AUTHORIZATION, http::header::ACCEPT])
                  .allowed_header(http::header::CONTENT_TYPE)
                  .max_age(3600)
                  .finish())
            .data(AppState {
                foo: String::from("{ \"Success\": { } }"),
                always_foo: false,
             })
            .service(api_index))
        .bind("127.0.0.1:8088")?
        .run()
        .await
}
