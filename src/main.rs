use serde::{Deserialize, Serialize};
use serde_json;
use solana_client::rpc_client::RpcClient;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::{program_pack::Pack, signer::Signer};
use solana_sdk::{
    pubkey::Pubkey, signer::keypair::Keypair, system_instruction::create_account,
    transaction::Transaction,
};
use spl_associated_token_account::{
    get_associated_token_address, instruction::create_associated_token_account,
};
use spl_token::{
    instruction::{initialize_account, initialize_mint, mint_to_checked, transfer_checked},
    state::Mint,
    ID,
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
#[derive(Serialize, Deserialize, Debug)]
struct PubkeyAndSignature {
    pubkey: String,
    signature: String,
}
#[derive(Serialize, Deserialize, Debug)]
struct CacheFile {
    token_mint: String,
    tokens_amount: usize,
    pubkeys_and_signatures: Vec<PubkeyAndSignature>,
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
fn create_cache(
    file_name: &String,
    pubkeys_and_signatures: Vec<PubkeyAndSignature>,
    token_mint: String,
) -> std::io::Result<()> {
    let path = env::current_dir().unwrap().display().to_string();
    let cache_path = format!("{path}/src/cache");
    if !std::path::Path::new(&cache_path).exists() {
        std::fs::create_dir(&cache_path)?;
    }
    let cache_content = CacheFile {
        token_mint,
        tokens_amount: pubkeys_and_signatures.len(),
        pubkeys_and_signatures,
    };

    let content_parsed = serde_json::to_string(&cache_content)?;

    let file_path = format!("{cache_path}/{file_name}");
    std::fs::write(file_path, content_parsed.as_bytes())?;
    println!("cache created");

    Ok(())
}
fn transactions(
    pubkeys: Vec<Pubkey>,
    provided_mint: Option<String>,
) -> (String, Vec<PubkeyAndSignature>) {
    let commitment_config = CommitmentConfig::processed();
    let rpc = RpcClient::new_with_commitment(
        "https://api.devnet.solana.com/".to_string(),
        commitment_config,
    );

    let space = Mint::LEN;
    let wallet = read_wallet().unwrap();
    let wallet_keypair = Keypair::from_bytes(&wallet[..]).unwrap();
    let mint_acc = Keypair::new();
    let mut mint_acc_pubkey: Pubkey = mint_acc.pubkey();
    let blockhash = rpc.get_latest_blockhash().unwrap();
    let token_acc: Pubkey;

    if let Some(mint) = provided_mint {
        println!("You provided a mint {}", mint);
        mint_acc_pubkey = Pubkey::from_str(&mint).unwrap();
        token_acc = get_associated_token_address(&wallet_keypair.pubkey(), &mint_acc_pubkey);

        let mint_instruction = mint_to_checked(
            &ID,
            &mint_acc_pubkey,
            &token_acc,
            &wallet_keypair.pubkey(),
            &[],
            pubkeys.len() as u64,
            0,
        )
        .unwrap();
        let tx = Transaction::new_signed_with_payer(
            &[mint_instruction],
            Some(&wallet_keypair.pubkey()),
            &[&wallet_keypair],
            blockhash,
        );
        rpc.send_and_confirm_transaction(&tx).unwrap();
    } else {
        let min_balance = rpc.get_minimum_balance_for_rent_exemption(space).unwrap();
        let token_account_ix = create_account(
            &wallet_keypair.pubkey(),
            &mint_acc_pubkey,
            min_balance,
            space as u64,
            &ID,
        );
        let token_mint_ix = initialize_mint(
            &ID,
            &mint_acc_pubkey,
            &wallet_keypair.pubkey(),
            Some(&wallet_keypair.pubkey()),
            0,
        )
        .unwrap();

        let tx = Transaction::new_signed_with_payer(
            &[token_account_ix, token_mint_ix],
            Some(&wallet_keypair.pubkey()),
            &[&wallet_keypair, &mint_acc],
            blockhash,
        );
        rpc.send_and_confirm_transaction(&tx).unwrap();
        token_acc = get_associated_token_address(&wallet_keypair.pubkey(), &mint_acc_pubkey);

        let mint_instruction = mint_to_checked(
            &ID,
            &mint_acc_pubkey,
            &token_acc,
            &wallet_keypair.pubkey(),
            &[],
            pubkeys.len() as u64,
            0,
        )
        .unwrap();

        let tx = Transaction::new_signed_with_payer(
            &[mint_instruction],
            Some(&wallet_keypair.pubkey()),
            &[&wallet_keypair],
            blockhash,
        );
        rpc.send_and_confirm_transaction(&tx).unwrap();
        println!("Mint created {}", mint_acc_pubkey.to_string());
    }

    let mut pubkeys_and_signatures: Vec<PubkeyAndSignature> = vec![];
    for (index, destination_pubkey) in pubkeys.iter().enumerate() {
        let destination_token_acc =
            get_associated_token_address(&destination_pubkey, &mint_acc_pubkey);
        let transfer_ix = transfer_checked(
            &ID,
            &token_acc,
            &mint_acc_pubkey,
            &destination_token_acc,
            &wallet_keypair.pubkey(),
            &[],
            1,
            0,
        )
        .unwrap();
        let tx = Transaction::new_signed_with_payer(
            &[transfer_ix],
            Some(&wallet_keypair.pubkey()),
            &[&wallet_keypair],
            blockhash,
        );

        let pubkey_and_signature = PubkeyAndSignature {
            pubkey: destination_pubkey.to_string(),
            signature: rpc.send_and_confirm_transaction(&tx).unwrap().to_string(),
        };

        pubkeys_and_signatures.push(pubkey_and_signature);
        println!("Token sent to {}", destination_pubkey.to_string());
        println!("{}% COMPLETED", ((index + 1) * 100) / pubkeys.len());
    }
    return (token_acc.to_string(), pubkeys_and_signatures);
}
fn main() -> std::io::Result<()> {
    let args: Vec<String> = env::args().collect();
    let path = env::current_dir().unwrap().display().to_string();
    let entries = read_dir(format!("{}/src/pubkeys/", path)).unwrap();
    for file in entries {
        let file_name = file.as_ref().unwrap().file_name().into_string().unwrap();
        let process = format!("Processing {} ~ Continue [y/n]?\n", file_name);
        std::io::stdout().write_all(process.as_bytes())?;
        let mut buffer = String::new();
        std::io::stdin().read_line(&mut buffer)?;
        if buffer.starts_with("n") || buffer.starts_with("N") {
            continue;
        };
        let mut file_content = File::open(file.unwrap().path())?;
        let mut contents = String::new();
        file_content.read_to_string(&mut contents)?;
        let content_parsed: Vec<Pubkeys> = serde_json::from_str(&contents)?;

        let mut provided_mint: Option<String> = None;
        if args.len() > 1 && !args[1].is_empty() {
            provided_mint = Some(args[1].clone());
        }
        let pubkeys: Vec<Pubkey> = content_parsed
            .iter()
            .map(|x| Pubkey::from_str(&x.id.clone()).unwrap())
            .collect();
        let (token_mint, pubkeys_and_signatures) = transactions(pubkeys, provided_mint);
        create_cache(&file_name, pubkeys_and_signatures, token_mint)?;
    }
    Ok(())
}
