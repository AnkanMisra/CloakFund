mod stealth;

use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 3 {
        eprintln!("Usage: {} <command> <args...>", args[0]);
        eprintln!("Commands:");
        eprintln!("  generate <recipient_pubkey>    Generate stealth address");
        eprintln!("  recover <recipient_priv> <ephemeral_pub>    Recover stealth private key");
        return;
    }

    match args[1].as_str() {
        "generate" => match stealth::generate_stealth_address(&args[2]) {
            Ok((addr, ephem, tag)) => {
                println!("Stealth Address: {}", addr);
                println!("Ephemeral Pubkey: {}", ephem);
                println!("View Tag: 0x{:02x}", tag);
            }
            Err(e) => eprintln!("Error: {}", e),
        },
        "recover" => {
            if args.len() < 4 {
                eprintln!("Missing arguments for recover");
                return;
            }
            match stealth::recover_stealth_private_key(&args[2], &args[3]) {
                Ok(priv_key) => println!("Stealth Private Key: {}", priv_key),
                Err(e) => eprintln!("Error: {}", e),
            }
        }
        _ => eprintln!("Unknown command: {}", args[1]),
    }
}
