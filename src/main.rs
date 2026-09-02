use regex::{Regex};
use std::{env, net::Ipv4Addr, process::{exit}};

fn yell() {
    println!("Incorrect usage. Do: syncho <player_name> <peer_ip>");
    exit(0)    
}

fn get_argument(pattern: &str, argument: &String) -> Option<String> {
    let matcher = Regex::new(pattern).unwrap();
    let result = matcher.find(argument).unwrap();

    if result.is_empty() {
        None
    } else {
        Some(result.as_str().to_string())
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
    if let None = peer_ip {
        yell();
    }

}
