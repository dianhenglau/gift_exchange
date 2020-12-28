use actix_files::NamedFile;
use actix_web::{get, post, web, App, HttpRequest, HttpResponse, HttpServer, Responder};
use askama_actix::Template;
use rand::Rng;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct WishNote {
    presents: Vec<String>,
    my_place: String,
    author_id: String,
    santa_id: String,
}

struct AppState {
    sessions: std::sync::Mutex<std::collections::HashMap<String, String>>,
    wish_notes: std::sync::RwLock<Vec<WishNote>>,
}

#[derive(Template)]
#[template(path = "hello.html")]
struct HelloTemplate<'a> {
    wish_notes: &'a Vec<WishNote>,
    filled: bool,
    id: String,
    draw_enabled: bool,
    fill_success: bool,
    draw_success: bool,
    drawn_note: Option<&'a WishNote>,
}

#[derive(Template)]
#[template(path = "error.html")]
struct ErrorTemplate {
    message: String,
}

#[derive(Deserialize)]
struct Query {
    id: Option<String>,
}

#[get("/")]
async fn hello(query: web::Query<Query>, data: web::Data<AppState>) -> impl Responder {
    let wish_notes = data.wish_notes.read().unwrap();

    let mut sessions = data.sessions.lock().unwrap();

    let id = match &query.id {
        Some(x) => x.clone(),
        None => {
            let id = String::from_utf8(
                rand::thread_rng()
                    .sample_iter(&rand::distributions::Alphanumeric)
                    .take(10)
                    .collect::<Vec<u8>>(),
            )
            .unwrap();

            if sessions.len() > wish_notes.len() + 1024 {
                sessions.clear();

                for note in wish_notes.iter() {
                    sessions.insert(note.author_id.clone(), String::new());
                }
            }

            sessions.insert(id.clone(), String::new());

            id
        }
    };

    let filled = wish_notes.iter().any(|x| x.author_id == id);

    let (fill_success, draw_success) = match sessions.get_mut(&id) {
        Some(x) => {
            let (a, b) = (x == "fill_success", x == "draw_success");
            x.clear();
            (a, b)
        }
        None => (false, false),
    };

    let drawn_note = wish_notes.iter().find(|x| x.santa_id == id);

    HttpResponse::Ok().body(
        HelloTemplate {
            wish_notes: &wish_notes,
            filled,
            id,
            draw_enabled: true,
            fill_success,
            draw_success,
            drawn_note,
        }
        .render()
        .unwrap(),
    )
}

#[derive(Deserialize)]
struct WishFormData {
    id: String,
    present1: String,
    present2: String,
    present3: String,
    my_place: String,
}

#[post("/submit_wish")]
async fn submit_wish(
    mut form: web::Form<WishFormData>,
    data: web::Data<AppState>,
) -> impl Responder {
    let id = std::mem::take(&mut form.id);

    let mut sessions = data.sessions.lock().unwrap();

    if !sessions.contains_key(&id) {
        return HttpResponse::Forbidden().body(
            ErrorTemplate {
                message: String::from("內部錯誤"),
            }
            .render()
            .unwrap(),
        );
    }

    let mut wish_notes = data.wish_notes.write().unwrap();

    if wish_notes.iter().any(|x| x.author_id == id) {
        return HttpResponse::BadRequest().body(
            ErrorTemplate {
                message: String::from("你已經投了"),
            }
            .render()
            .unwrap(),
        );
    }

    if form.present1.is_empty() {
        return HttpResponse::UnprocessableEntity().body(
            ErrorTemplate {
                message: String::from("禮物一是必填的"),
            }
            .render()
            .unwrap(),
        );
    }

    if form.present2.is_empty() {
        return HttpResponse::UnprocessableEntity().body(
            ErrorTemplate {
                message: String::from("禮物二是必填的"),
            }
            .render()
            .unwrap(),
        );
    }

    if form.present3.is_empty() {
        return HttpResponse::UnprocessableEntity().body(
            ErrorTemplate {
                message: String::from("禮物三是必填的"),
            }
            .render()
            .unwrap(),
        );
    }

    if form.my_place.is_empty() {
        return HttpResponse::UnprocessableEntity().body(
            ErrorTemplate {
                message: String::from("不曾到過的地方是必填的"),
            }
            .render()
            .unwrap(),
        );
    }

    let wish_note = WishNote {
        presents: vec![
            std::mem::take(&mut form.present1),
            std::mem::take(&mut form.present2),
            std::mem::take(&mut form.present3),
        ],
        my_place: std::mem::take(&mut form.my_place),
        author_id: id.clone(),
        santa_id: String::new(),
    };

    std::fs::write(
        format!("data/wishes/{}", &id),
        serde_json::to_string(&wish_note).unwrap(),
    )
    .unwrap();

    wish_notes.push(wish_note);

    sessions
        .get_mut(&id)
        .unwrap()
        .replace_range(.., "fill_success");

    HttpResponse::SeeOther()
        .set_header("Location", format!("/?id={}", &id))
        .finish()
}

#[derive(Deserialize)]
struct DrawFormData {
    id: String,
}

#[post("/draw")]
async fn draw(mut form: web::Form<DrawFormData>, data: web::Data<AppState>) -> impl Responder {
    let id = std::mem::take(&mut form.id);

    let mut wish_notes = data.wish_notes.write().unwrap();

    if !wish_notes.iter().any(|x| x.author_id == id) {
        return HttpResponse::Forbidden().body(
            ErrorTemplate {
                message: String::from("你還沒交上願望紙條"),
            }
            .render()
            .unwrap(),
        );
    }

    if wish_notes.iter().any(|x| x.santa_id == id) {
        return HttpResponse::BadRequest().body(
            ErrorTemplate {
                message: String::from("你已經抽過了"),
            }
            .render()
            .unwrap(),
        );
    }

    let avail_ids = wish_notes
        .iter()
        .filter(|x| x.santa_id.is_empty())
        .map(|x| x.author_id.clone())
        .collect::<Vec<String>>();

    if avail_ids.is_empty() {
        return HttpResponse::BadRequest().body(
            ErrorTemplate {
                message: String::from("已經都抽完了"),
            }
            .render()
            .unwrap(),
        );
    }

    let picked_id = if avail_ids.len() == 1 {
        &avail_ids[0]

    } else if avail_ids.len() == 2 {
        if id == avail_ids[0] {
            &avail_ids[1]
        } else {
            &avail_ids[0]
        }

    } else {
        let mut rng = rand::thread_rng();

        loop {
            let x = avail_ids.get(rng.gen_range(0..avail_ids.len())).unwrap();

            if x != &id {
                break x;
            }
        }
    };

    let note = wish_notes
        .iter_mut()
        .find(|x| &x.author_id == picked_id)
        .unwrap();

    note.santa_id.replace_range(.., &id);

    std::fs::write(
        format!("data/wishes/{}", &note.author_id),
        serde_json::to_string(&note).unwrap(),
    )
    .unwrap();

    let mut sessions = data.sessions.lock().unwrap();

    sessions
        .get_mut(&id)
        .unwrap()
        .replace_range(.., "draw_success");

    HttpResponse::SeeOther()
        .set_header("Location", format!("/?id={}", &id))
        .finish()
}

#[get("/{filename:.*}")]
async fn serve_static_files(req: HttpRequest) -> actix_web::Result<NamedFile> {
    let path: std::path::PathBuf = ["static", req.match_info().query("filename")]
        .iter()
        .collect();

    Ok(NamedFile::open(path)?)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let wish_notes = std::fs::read_dir("data/wishes")
        .unwrap()
        .map(|x| {
            serde_json::from_str::<WishNote>(&std::fs::read_to_string(x.unwrap().path()).unwrap())
                .unwrap()
        })
        .collect::<Vec<WishNote>>();

    let mut sessions = std::collections::HashMap::<String, String>::new();

    for note in &wish_notes {
        sessions.insert(note.author_id.clone(), String::new());
    }

    let state = web::Data::new(AppState {
        sessions: std::sync::Mutex::new(sessions),
        wish_notes: std::sync::RwLock::new(wish_notes),
    });

    HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .service(hello)
            //.service(submit_wish)
            .service(draw)
            .service(serve_static_files)
    })
    .bind("0.0.0.0:80")?
    .run()
    .await
}
