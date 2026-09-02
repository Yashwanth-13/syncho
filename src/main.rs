use regex::{Match, Regex};
use std::{env, f32::consts::E, fmt::Error, net::Ipv4Addr, process::{Termination, exit}, ptr::read};

use cider_api::CiderClient;

fn yell() {
    println!("Incorrect usage. Do: syncho <player_name> <peer_ip>");
    exit(0)    
}

fn parse_ipv4(s: &str) -> Result<Ipv4Addr, String> {
    let matched = 
    
}

fn get_argument(pattern: &str, argument: &String) -> Option<&str> {
    let matcher = Regex::new(&pattern).unwrap();
    let result = matcher.find(argument).unwrap();

    if result.is_empty() {
        None
    } else {
        Some(result.as_str())
    }

}


fn main() {

    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        yell();
    }
    
    let player = get_argument("(?i)(?:cider|spotify)", args.get(1).unwrap());
    if let None = player {
        yell();
    }

    let peer_ip = args.get(2).unwrap().parse::<Ipv4Addr>().ok();
}


fn start_player(player: String) {
    if player == "cider" {

    }
}