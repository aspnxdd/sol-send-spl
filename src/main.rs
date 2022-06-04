use serde::{Deserialize, Serialize};
use serde_json;
use solana_client::rpc_client::RpcClient;
use solana_sdk::{program_pack::Pack, signer::Signer};
use solana_sdk::{
    pubkey::Pubkey, signer::keypair::Keypair, system_instruction::create_account,
    transaction::Transaction,
};
use spl_associated_token_account::{
    get_associated_token_address, instruction::create_associated_token_account,
};
use spl_token::{
    id,
    instruction::{initialize_mint, mint_to_checked, transfer_checked},
    state::Mint,
};
use std::io::prelude::*;
use std::str::FromStr;
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

fn read_wallet() -> std::io::Result<Vec<u8>> {
    let path = env::current_dir().unwrap().display().to_string();
    let wallet_file_name = format!("{}/src/wallet.json", path);
    let mut file_content = File::open(wallet_file_name).unwrap_or_else(|error| {
        panic!("File not found: {:?}", error);
    });
    let mut content = String::new();
    file_content.read_to_string(&mut content)?;
    let wallet: Vec<u8> = serde_json::from_str(&content)?;
    return Ok(wallet);
}

fn transactions(pubkeys: Vec<Pubkey>, provided_mint: Option<String>) {
    let rpc = RpcClient::new("https://api.devnet.solana.com/".to_string());
    let space = Mint::LEN;
    let wallet = read_wallet().unwrap();
    let wallet_keypair = Keypair::from_bytes(&wallet[..]).unwrap();
    let mut mint_acc = Keypair::new().pubkey();
    let mut token_acc = Keypair::new().pubkey();
    if let Some(mint) = provided_mint {
        println!("You provided a mint {}", mint);
        mint_acc = Pubkey::from_str(&mint).unwrap();
        token_acc = get_associated_token_address(&wallet_keypair.pubkey(), &mint_acc);
    }

    let min_balance = rpc.get_minimum_balance_for_rent_exemption(space).unwrap();
    let token_account_ix = create_account(
        &wallet_keypair.pubkey(),
        &mint_acc,
        min_balance,
        space as u64,
        &id(),
    );

    let token_mint_ix = initialize_mint(
        &id(),
        &mint_acc,
        &wallet_keypair.pubkey(),
        Some(&wallet_keypair.pubkey()),
        0,
    )
    .unwrap();

    let mint_instruction = mint_to_checked(
        &id(),
        &mint_acc,
        &token_acc,
        &wallet_keypair.pubkey(),
        &[],
        1,
        0,
    )
    .unwrap();

    let _tx = Transaction::new_with_payer(
        &[token_account_ix, token_mint_ix, mint_instruction],
        Some(&wallet_keypair.pubkey()),
    );
    println!("mint_acc {}", mint_acc.to_string());
    println!("token_acc {}", token_acc.to_string());
    println!("wallet_keypair {}", wallet_keypair.pubkey().to_string());

    for destination_pubkey in pubkeys {
        let create_ata_ix = create_associated_token_account(
            &wallet_keypair.pubkey(),
            &destination_pubkey,
            &mint_acc,
        );
        let transfer_ix = transfer_checked(
            &id(),
            &wallet_keypair.pubkey(),
            &mint_acc,
            &destination_pubkey,
            &wallet_keypair.pubkey(),
            &[&wallet_keypair.pubkey(), &mint_acc],
            1,
            0,
        )
        .unwrap();
        let tx = Transaction::new_with_payer(
            &[create_ata_ix, transfer_ix],
            Some(&wallet_keypair.pubkey()),
        );
        println!("tx {:#?}", tx);
    }
}
fn main() -> std::io::Result<()> {
    let args: Vec<String> = env::args().collect();
    let path = env::current_dir().unwrap().display().to_string();
    let entries = read_dir(format!("{}/src/pubkeys/", path)).unwrap();

    for file in entries {
        let mut file_content = File::open(file.unwrap().path())?;
        let mut contents = String::new();
        file_content.read_to_string(&mut contents)?;
        let content_parsed: Vec<Pubkeys> = serde_json::from_str(&contents)?;

        let mut provided_mint: Option<String> = None;
        if !args[1].is_empty() {
            provided_mint = Some(args[1].clone());
        }
        let pubkeys: Vec<Pubkey> = content_parsed
            .iter()
            .map(|x| Pubkey::from_str(&x.id.clone()).unwrap())
            .collect();
        transactions(pubkeys, provided_mint);
    }
    Ok(())
}
