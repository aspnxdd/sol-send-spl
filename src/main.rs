use serde::{Deserialize, Serialize};
use serde_json;
use solana_client::rpc_client::RpcClient;
use solana_sdk::program_pack::Pack;
use solana_sdk::signer::Signer;
use solana_sdk::{
    signer::keypair::Keypair, system_instruction::create_account, transaction::Transaction,
};
use spl_token::{
    id,
    instruction::{initialize_mint, mint_to_checked},
    state::Mint,
};
use std::fs::ReadDir;
use std::io::prelude::*;
use std::{
    env,
    fs::{read_dir, File},
};

#[derive(Serialize, Deserialize, Debug)]
struct NFTs {
    id: String,
}
#[derive(Serialize, Deserialize, Debug)]
struct Pubkeys {
    id: String,
    nfts: Vec<NFTs>,
}

fn read_pubkeys(entries: ReadDir) -> std::io::Result<()> {
    for file in entries {
        // println!("Name: {}", file.unwrap().path().display());
        let mut file_content = File::open(file.unwrap().path())?;
        let mut contents = String::new();
        file_content.read_to_string(&mut contents)?;
        let content_parsed: Vec<Pubkeys> = serde_json::from_str(&contents)?;
        let pubkeys: Vec<String> = content_parsed.iter().map(|x| x.id.clone()).collect();
        println!("123 {:#?}", pubkeys);
    }
    Ok(())
}
fn main() -> std::io::Result<()> {
    let path = env::current_dir().unwrap().display().to_string();
    println!("1 {}", path);
    let entries = read_dir(format!("{}/src/pubkeys/", path)).unwrap();

    read_pubkeys(entries)?;

    let rpc = RpcClient::new("https://api.devnet.solana.com/".to_string());

    println!("2");
    let space = Mint::LEN;
    let recent_blockhash = rpc.get_latest_blockhash().unwrap();

    let wallet = Keypair::new();
    println!("33");

    let mint_acc = Keypair::new();
    let token_acc = Keypair::new();
    println!("35");
    rpc.request_airdrop(&wallet.pubkey(), 2000000000).unwrap();
    let min_balance = rpc.get_minimum_balance_for_rent_exemption(space).unwrap();
    let token_account_ix = create_account(
        &wallet.pubkey(),
        &mint_acc.pubkey(),
        min_balance,
        space as u64,
        &id(),
    );
    println!("3");

    let token_mint_ix = initialize_mint(
        &id(),
        &mint_acc.pubkey(),
        &wallet.pubkey(),
        Some(&wallet.pubkey()),
        0,
    )
    .unwrap();

    println!("4");

    let mint_instruction = mint_to_checked(
        &id(),
        &mint_acc.pubkey(),
        &token_acc.pubkey(),
        &wallet.pubkey(),
        &[],
        1,
        0,
    )
    .unwrap();

    let tx = Transaction::new_signed_with_payer(
        &[token_account_ix, token_mint_ix, mint_instruction],
        Some(&wallet.pubkey()),
        &[&wallet, &mint_acc],
        recent_blockhash,
    );
    println!("mint_acc {}", mint_acc.pubkey().to_string());
    println!("token_acc {}", token_acc.pubkey().to_string());

    println!("$ {}", wallet.pubkey().to_string());
    Ok(())
}
