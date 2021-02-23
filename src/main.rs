// Copyright 2019 Federico Fissore
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#[macro_use]
extern crate serde_derive;

use std::{env, fs};
use std::net::Ipv4Addr;
use std::net::Ipv6Addr;
use std::sync::Arc;

use actix_cors::Cors;
use actix_web::http::HeaderMap;
use actix_web::web;
use actix_web::App;
use actix_web::HttpRequest;
use actix_web::HttpResponse;
use actix_web::HttpServer;
use maxminddb::geoip2::City;
use maxminddb::MaxMindDBError;
use maxminddb::Reader;
use memmap::Mmap;
use serde_json::Value;

#[derive(Serialize)]
struct NonResolvedIPResponse<'a> {
    pub ip_address: &'a str,
}

#[derive(Serialize)]
struct ResolvedIPResponse<'a> {
    pub ipAddress: &'a str,
    pub latitude: &'a f64,
    pub longitude: &'a f64,
    pub postalCode: &'a str,
    pub continentCode: &'a str,
    pub continentName: &'a str,
    pub countryCode: &'a str,
    pub countryLabel: &'a str,
    pub countryName: &'a str,
    pub regionCode: &'a str,
    pub regionName: &'a str,
    pub provinceCode: &'a str,
    pub provinceName: &'a str,
    pub cityName: &'a str,
    pub timeZone: &'a str,
}

#[derive(Deserialize, Debug)]
struct QueryParams {
    ip: Option<String>,
    lang: Option<String>,
    callback: Option<String>,
}

fn ip_address_to_resolve(
    ip: Option<String>,
    headers: &HeaderMap,
    remote_addr: Option<&str>,
) -> String {
    ip.filter(|ip_address| {
        ip_address.parse::<Ipv4Addr>().is_ok() || ip_address.parse::<Ipv6Addr>().is_ok()
    })
        .or_else(|| {
            headers
                .get("X-Real-IP")
                .map(|s| s.to_str().unwrap().to_string())
        })
        .or_else(|| {
            remote_addr
                .map(|ip_port| ip_port.split(':').take(1).last().unwrap())
                .map(|ip| ip.to_string())
        })
        .expect("unable to find ip address to resolve")
}

fn get_language(lang: Option<String>) -> String {
    lang.unwrap_or_else(|| String::from("en"))
}

fn get_localized_country_name(lang: &str, code: &str) -> String {
    return if let Ok(path) = env::var("GEOIP_RS_COUNTRY_NAMES") {
        let _file = fs::read_to_string(path).unwrap();
        get_value(_file, lang, code)
    } else {
        String::from("")
    };
}

fn get_value(file: String, lang: &str, code: &str) -> String {
    let content = file.parse::<Value>().unwrap();
    if content[lang][code].is_null() {
        String::from("")
    } else {
        content[lang][code].as_str().unwrap().to_string()
    }
}

struct Db {
    db: Arc<Reader<Mmap>>,
}

async fn index(req: HttpRequest, data: web::Data<Db>, web::Query(query): web::Query<QueryParams>) -> HttpResponse {
    let language = get_language(query.lang);
    let ip_address = ip_address_to_resolve(query.ip, req.headers(), req.connection_info().remote());

    let lookup: Result<City, MaxMindDBError> = data.db.lookup(ip_address.parse().unwrap());

    let geoip = match lookup {
        Ok(geoip) => {
            let region = geoip
                .subdivisions
                .as_ref()
                .filter(|subdivs| !subdivs.is_empty())
                .and_then(|subdivs| subdivs.get(0));

            let province = geoip
                .subdivisions
                .as_ref()
                .filter(|subdivs| subdivs.len() > 1)
                .and_then(|subdivs| subdivs.get(1));

            let localize_country_name = get_localized_country_name(&language, geoip.country.as_ref()
                .and_then(|country| country.iso_code.as_ref())
                .map(String::as_str)
                .unwrap_or(""));

            let res = ResolvedIPResponse {
                ipAddress: &ip_address,
                latitude: geoip
                    .location
                    .as_ref()
                    .and_then(|loc| loc.latitude.as_ref())
                    .unwrap_or(&0.0),
                longitude: geoip
                    .location
                    .as_ref()
                    .and_then(|loc| loc.longitude.as_ref())
                    .unwrap_or(&0.0),
                postalCode: geoip
                    .postal
                    .as_ref()
                    .and_then(|postal| postal.code.as_ref())
                    .map(String::as_str)
                    .unwrap_or(""),
                continentCode: geoip
                    .continent
                    .as_ref()
                    .and_then(|cont| cont.code.as_ref())
                    .map(String::as_str)
                    .unwrap_or(""),
                continentName: geoip
                    .continent
                    .as_ref()
                    .and_then(|cont| cont.names.as_ref())
                    .and_then(|names| names.get("en"))
                    .map(String::as_str)
                    .unwrap_or(""),
                countryCode: geoip
                    .country
                    .as_ref()
                    .and_then(|country| country.iso_code.as_ref())
                    .map(String::as_str)
                    .unwrap_or(""),
                countryLabel: geoip
                    .country
                    .as_ref()
                    .and_then(|country| country.names.as_ref())
                    .and_then(|names| names.get(&language))
                    .map(String::as_str)
                    .unwrap_or(&localize_country_name),
                countryName: geoip
                    .country
                    .as_ref()
                    .and_then(|country| country.names.as_ref())
                    .and_then(|names| names.get("en"))
                    .map(String::as_str)
                    .unwrap_or(&localize_country_name),
                regionCode: region
                    .and_then(|subdiv| subdiv.iso_code.as_ref())
                    .map(String::as_ref)
                    .unwrap_or(""),
                regionName: region
                    .and_then(|subdiv| subdiv.names.as_ref())
                    .and_then(|names| names.get("en"))
                    .map(String::as_ref)
                    .unwrap_or(""),
                provinceCode: province
                    .and_then(|subdiv| subdiv.iso_code.as_ref())
                    .map(String::as_ref)
                    .unwrap_or(""),
                provinceName: province
                    .and_then(|subdiv| subdiv.names.as_ref())
                    .and_then(|names| names.get("en"))
                    .map(String::as_ref)
                    .unwrap_or(""),
                cityName: geoip
                    .city
                    .as_ref()
                    .and_then(|city| city.names.as_ref())
                    .and_then(|names| names.get("en"))
                    .map(String::as_str)
                    .unwrap_or(""),
                timeZone: geoip
                    .location
                    .as_ref()
                    .and_then(|loc| loc.time_zone.as_ref())
                    .map(String::as_str)
                    .unwrap_or(""),
            };
            serde_json::to_string(&res)
        }
        Err(_) => serde_json::to_string(&NonResolvedIPResponse {
            ip_address: &ip_address,
        }),
    }
        .unwrap();

    match query.callback {
        Some(callback) => HttpResponse::Ok()
            .content_type("application/javascript; charset=utf-8")
            .body(format!(";{}({});", callback, geoip)),
        None => HttpResponse::Ok()
            .content_type("application/json; charset=utf-8")
            .body(geoip),
    }
}

fn db_file_path() -> String {
    if let Ok(file) = env::var("GEOIP_RS_DB_PATH") {
        return file;
    }

    let args: Vec<String> = env::args().collect();
    if args.len() > 1 {
        return args[1].to_string();
    }

    panic!("You must specify the db path, either as a command line argument or as GEOIP_RS_DB_PATH env var");
}

#[actix_rt::main]
async fn main() {
    dotenv::from_path(".env").ok();

    let host = env::var("GEOIP_RS_HOST").unwrap_or_else(|_| String::from("127.0.0.1"));
    let port = env::var("GEOIP_RS_PORT").unwrap_or_else(|_| String::from("3000"));

    println!("Listening on http://{}:{}", host, port);

    let db = Arc::new(Reader::open_mmap(db_file_path()).unwrap());

    HttpServer::new(move || {
        App::new()
            .data(Db { db: db.clone() })
            .wrap(Cors::new().send_wildcard().finish())
            .route("/", web::route().to(index))
    })
        .bind(format!("{}:{}", host, port))
        .unwrap_or_else(|_| panic!("Can not bind to {}:{}", host, port))
        .run()
        .await
        .unwrap();
}
