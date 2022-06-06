use clap::Parser;
use serde::{Deserialize, Serialize};
use serde_json;
use solana_client::rpc_client::RpcClient;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::{
    client::SyncClient, pubkey::Pubkey, signer::keypair::Keypair,
    system_instruction::create_account, transaction::Transaction,
};
use solana_sdk::{program_pack::Pack, signer::Signer};
use spl_associated_token_account::get_associated_token_address;
use spl_associated_token_account::instruction::create_associated_token_account;
use spl_token::{
    instruction::{initialize_mint, mint_to_checked, transfer_checked, TokenInstruction},
    state::Mint,
    ID,
};
use std::io;
use std::io::{prelude::*, Result};
use std::str::FromStr;
use std::{
    env::current_dir,
    fs,
    fs::{read_dir, File},
    path,
};
#[derive(Parser, Debug)]
#[clap(name = "SPL Token Airdropper from Matrica")]
#[clap(author = "Arnau E. <aespin@boxfish.studio>")]
#[clap(version = "1.0")]
#[clap(about = "Tool to airdrop SPL tokens to the accounts provided from matrica as .json files.", long_about = None)]

struct Args {
    /// <bool> - should check spl token amount after airdrop, to make sure the airdrop succeeded.
    #[clap(short, long)]
    should_check_spl_amount: bool,

    /// <string> - if you have the authority for a mint, add it so you can airdrop it instead of generating a new random mint.
    #[clap(short, long)]
    mint_address: Option<String>,
}

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
    amount: Option<u8>,
    signature: String,
}
#[derive(Serialize, Deserialize, Debug)]
struct CacheFile {
    token_mint: String,
    tokens_amount: usize,
    pubkeys_and_signatures: Vec<PubkeyAndSignature>,
}

pub const SPACE: usize = Mint::LEN;
pub const RPC_ENDPOINT: &str = "https://api.devnet.solana.com/";

fn read_wallet() -> Result<Vec<u8>> {
    let wallet_file_name = format!("{}/src/wallet.json", current_dir()?.display().to_string());
    let mut file_content = File::open(wallet_file_name).unwrap_or_else(|_| {
        panic!("wallet.json not found, please place you wallet.json in ~/src/<wallet.json>");
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
) -> Result<()> {
    let cache_path = format!("{}/src/cache", current_dir()?.display().to_string());
    if !path::Path::new(&cache_path).exists() {
        fs::create_dir(&cache_path)?;
    }
    let cache_content = CacheFile {
        token_mint,
        tokens_amount: pubkeys_and_signatures.len(),
        pubkeys_and_signatures,
    };

    let content_parsed = serde_json::to_string(&cache_content)?;

    let file_path = format!("{cache_path}/{file_name}");
    fs::write(file_path, content_parsed.as_bytes())?;
    println!("cache created");

    Ok(())
}
fn transactions(
    rpc: &RpcClient,
    wallet_keypair: &Keypair,
    pubkeys: Vec<Pubkey>,
    provided_mint: &Option<String>,
    should_check_spl_amount: &bool,
) -> Result<(String, Vec<PubkeyAndSignature>)> {
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
            &[wallet_keypair],
            blockhash,
        );
        rpc.send_and_confirm_transaction(&tx).unwrap();
    } else {
        let min_balance = rpc.get_minimum_balance_for_rent_exemption(SPACE).unwrap();
        let token_account_ix = create_account(
            &wallet_keypair.pubkey(),
            &mint_acc_pubkey,
            min_balance,
            SPACE as u64,
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
            &[wallet_keypair, &mint_acc],
            blockhash,
        );
        rpc.send_and_confirm_transaction(&tx).unwrap();
        let create_ata_ix = create_associated_token_account(
            &wallet_keypair.pubkey(),
            &wallet_keypair.pubkey(),
            &mint_acc.pubkey(),
        );

        let tx = Transaction::new_signed_with_payer(
            &[create_ata_ix],
            Some(&wallet_keypair.pubkey()),
            &[wallet_keypair],
            blockhash,
        );
        rpc.send_and_confirm_transaction(&tx).unwrap();
        token_acc = get_associated_token_address(&wallet_keypair.pubkey(), &mint_acc_pubkey);
        println!("token acc {}", token_acc.to_string());
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
            &[wallet_keypair],
            blockhash,
        );
        rpc.send_and_confirm_transaction(&tx).unwrap();
        println!("Mint created {}", mint_acc_pubkey.to_string());
    }

    let mut pubkeys_and_signatures: Vec<PubkeyAndSignature> = vec![];
    for (index, destination_pubkey) in pubkeys.iter().enumerate() {
        let destination_token_acc =
            get_associated_token_address(&destination_pubkey, &mint_acc_pubkey);

        let create_ata_ix = create_associated_token_account(
            &wallet_keypair.pubkey(),
            &destination_pubkey,
            &mint_acc.pubkey(),
        );

        let tx = Transaction::new_signed_with_payer(
            &[create_ata_ix],
            Some(&wallet_keypair.pubkey()),
            &[wallet_keypair],
            blockhash,
        );
        rpc.send_and_confirm_transaction(&tx).unwrap_or_default();

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
            &[wallet_keypair],
            blockhash,
        );
        let signature = rpc.send_and_confirm_transaction(&tx).unwrap().to_string();
        let mut amount: Option<u8> = None;
        if *should_check_spl_amount {
            amount = check_spl_amount(rpc, destination_pubkey, mint_acc_pubkey);
        }

        let pubkey_and_signature = PubkeyAndSignature {
            pubkey: destination_pubkey.to_string(),
            signature,
            amount,
        };

        pubkeys_and_signatures.push(pubkey_and_signature);
        println!("Token sent to {}", destination_pubkey.to_string());
        println!("{}% COMPLETED", ((index + 1) * 100) / pubkeys.len());
    }
    if pubkeys_and_signatures.is_empty() {
        panic!("Something went wrong, pubkeys_and_signatures is an empty vector.");
    }
    return Ok((token_acc.to_string(), pubkeys_and_signatures));
}

fn loop_files(args: Args) -> Result<()> {
    let commitment_config = CommitmentConfig::processed();
    let rpc = RpcClient::new_with_commitment(RPC_ENDPOINT, commitment_config);

    let wallet = read_wallet().unwrap();
    let wallet_keypair = Keypair::from_bytes(&wallet[..]).unwrap();
    let entries = read_dir(format!(
        "{}/src/pubkeys/",
        current_dir()?.display().to_string()
    ))
    .unwrap_or_else(|_| {
        panic!("Can't read pubkeys files in ~/src/pubkeys/<HERE>");
    });
    for file in entries {
        let file_name = file.as_ref().unwrap().file_name().into_string().unwrap();
        let process = format!("Processing {} ~ Continue [y/n]?\n", file_name);
        io::stdout().write_all(process.as_bytes())?;
        let mut buffer = String::new();
        io::stdin().read_line(&mut buffer)?;
        if buffer.starts_with("n") || buffer.starts_with("N") {
            continue;
        };
        let mut file_content = File::open(file.unwrap().path())?;
        let mut contents = String::new();
        file_content.read_to_string(&mut contents)?;
        let content_parsed: Vec<Pubkeys> = serde_json::from_str(&contents)?;

        let Args {
            mint_address: provided_mint,
            should_check_spl_amount,
        } = &args;
        let pubkeys: Vec<Pubkey> = content_parsed
            .iter()
            .map(|x| Pubkey::from_str(&x.id.clone()).unwrap())
            .collect();
        if let Ok((token_mint, pubkeys_and_signatures)) = transactions(
            &rpc,
            &wallet_keypair,
            pubkeys,
            &provided_mint,
            should_check_spl_amount,
        ) {
            create_cache(&file_name, pubkeys_and_signatures, token_mint)?;
        }
    }
    Ok(())
}

fn check_spl_amount(rpc: &RpcClient, pubkey: &Pubkey, mint_acc_pubkey: Pubkey) -> Option<u8> {
    let token_acc = get_associated_token_address(&pubkey, &mint_acc_pubkey);
    if let Ok(account_data) = rpc.get_token_account_balance(&token_acc) {
        println!(
            "Token amount for {} is: {}",
            pubkey.to_string(),
            account_data.amount
        );
        return Some(account_data.amount.parse().unwrap());
    }
    None
}
fn main() -> Result<()> {
    let args = Args::parse();
    if let Ok(_) = loop_files(args) {
        println!("Completed!");
    }

    Ok(())
}
