use {
    anchor_lang::{
        prelude::msg,
        solana_program::{instruction::Instruction, program_pack::Pack},
        system_program::ID as SYSTEM_PROGRAM_ID,
        AccountDeserialize, InstructionData, Key, ToAccountMetas,
    },
    anchor_spl::{
        associated_token::{self, ID as ASSOCIATED_TOKEN_PROGRAM_ID},
        token::spl_token,
    },
    litesvm::LiteSVM,
    litesvm_token::{
        spl_token::ID as TOKEN_PROGRAM_ID, CreateAssociatedTokenAccount, CreateMint, MintTo,
    },
    solana_keypair::{Address, Keypair},
    solana_message::Message,
    solana_pubkey::{pubkey, Pubkey},
    solana_signer::Signer,
    solana_transaction::Transaction,
};

// The sysvar clock program address
const CLOCK_SYSVAR_ID: Pubkey = pubkey!("SysvarC1ock11111111111111111111111111111111");

// Setup function to initialize LiteSVM and create a payer keypair
fn setup() -> (
    LiteSVM,
    Keypair,
    Address,
    Keypair,
    Address,
    Address,
    Address,
    Address,
    Address,
    Address,
    Address,
    Address,
) {
    let program_id = escrowq32026::id();
    let payer = Keypair::new();
    let mut svm = LiteSVM::new();
    let bytes = include_bytes!(concat!(
        env!("CARGO_TARGET_TMPDIR"),
        "/../deploy/escrowq32026.so"
    ));
    svm.add_program(program_id, bytes).unwrap();
    svm.airdrop(&payer.pubkey(), 1_000_000_000).unwrap();

    let maker = payer.pubkey();
    // Create two mints (Mint A and Mint B) with 6 decimal places and the maker as the authority
    // This done using litesvm-token's CreateMint utility which creates the mint in the LiteSVM environment
    let mint_a = CreateMint::new(&mut svm, &payer)
        .decimals(6)
        .authority(&maker)
        .send()
        .unwrap();
    msg!("Mint A: {}\n", mint_a);

    let mint_b = CreateMint::new(&mut svm, &payer)
        .decimals(6)
        .authority(&maker)
        .send()
        .unwrap();
    msg!("Mint B: {}\n", mint_b);

    // Create the maker's associated token account for Mint A
    // This is done using litesvm-token's CreateAssociatedTokenAccount utility
    let maker_ata_a = CreateAssociatedTokenAccount::new(&mut svm, &payer, &mint_a)
        .owner(&maker)
        .send()
        .unwrap();
    msg!("Maker ATA A: {}\n", maker_ata_a);

    let maker_ata_b = CreateAssociatedTokenAccount::new(&mut svm, &payer, &mint_b)
        .owner(&maker)
        .send()
        .unwrap();
    msg!("Maker ATA B: {}\n", maker_ata_b);

    // Derive the PDA for the escrow account using the maker's public key and a seed value
    let escrow = Pubkey::find_program_address(
        &[b"escrow", maker.as_ref(), &123u64.to_le_bytes()],
        &escrowq32026::id(),
    )
    .0;
    msg!("Escrow PDA: {}\n", escrow);

    // Derive the PDA for the vault associated token account using the escrow PDA and Mint A
    let vault = associated_token::get_associated_token_address(&escrow, &mint_a);
    msg!("Vault PDA: {}\n", vault);

    // Mint 1,000 tokens (with 6 decimal places) of Mint A to the maker's associated token account
    MintTo::new(&mut svm, &payer, &mint_a, &maker_ata_a, 1000_000_000)
        .send()
        .unwrap();

    let taker = Keypair::new();
    svm.airdrop(&taker.pubkey(), 1_000_000_000).unwrap();

    let taker_ata_a = CreateAssociatedTokenAccount::new(&mut svm, &taker, &mint_a)
        .owner(&taker.pubkey())
        .send()
        .unwrap();
    msg!("Taker ATA A: {}\n", taker_ata_a);

    let taker_ata_b = CreateAssociatedTokenAccount::new(&mut svm, &taker, &mint_b)
        .owner(&taker.pubkey())
        .send()
        .unwrap();
    msg!("Taker ATA B: {}\n", taker_ata_b);

    // Return the LiteSVM instance and payer keypair
    (
        svm,
        payer,
        maker,
        taker,
        mint_a,
        mint_b,
        maker_ata_a,
        maker_ata_b,
        taker_ata_a,
        taker_ata_b,
        escrow,
        vault,
    )
}

#[test]
fn test_make_and_refund() {
    // Setup the test environment by initializing LiteSVM and creating a payer keypair
    let (
        mut program,
        payer,
        maker,
        _taker,
        mint_a,
        mint_b,
        maker_ata_a,
        _maker_ata_b,
        _taker_ata_a,
        _taker_ata_b,
        escrow,
        vault,
    ) = setup();

    // Create the "Make" instruction to deposit tokens into the escrow
    let make_ix = Instruction {
        program_id: escrowq32026::id(),
        accounts: escrowq32026::accounts::Make {
            maker: maker,
            mint_a: mint_a,
            mint_b: mint_b,
            maker_ata_a: maker_ata_a,
            escrow: escrow,
            vault: vault,
            associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
            token_program: TOKEN_PROGRAM_ID,
            system_program: SYSTEM_PROGRAM_ID,
        }
        .to_account_metas(None),
        data: escrowq32026::instruction::Make {
            deposit: 10_000_000,
            seed: 123u64,
            receive: 10_000_000,
            expiration: 17780206209,
        }
        .data(),
    };

    // Create and send the transaction containing the "Make" instruction
    let message = Message::new(&[make_ix], Some(&payer.pubkey()));
    let recent_blockhash = program.latest_blockhash();

    let transaction = Transaction::new(&[&payer], message, recent_blockhash);

    // Send the transaction and capture the result
    let tx = program.send_transaction(transaction).unwrap();

    // Log transaction details
    msg!("\n\nMake transaction sucessfull");
    msg!("CUs Consumed: {}", tx.compute_units_consumed);
    msg!("Tx Signature: {}", tx.signature);

    // Verify the vault account and escrow account data after the "Make" instruction
    let vault_account = program.get_account(&vault).unwrap();
    let vault_data = spl_token::state::Account::unpack(&vault_account.data).unwrap();
    assert_eq!(vault_data.amount, 10_000_000);
    assert_eq!(vault_data.owner, escrow);
    assert_eq!(vault_data.mint, mint_a);

    let escrow_account = program.get_account(&escrow).unwrap();
    let escrow_data =
        escrowq32026::state::Escrow::try_deserialize(&mut escrow_account.data.as_ref()).unwrap();
    assert_eq!(escrow_data.seed, 123u64);
    assert_eq!(escrow_data.maker, maker);
    assert_eq!(escrow_data.mint_a, mint_a);
    assert_eq!(escrow_data.mint_b, mint_b);
    assert_eq!(escrow_data.receive, 10_000_000);

    // Create the "Refund" instruction to refund tokens back to the maker
    let refund_ix = Instruction {
        program_id: escrowq32026::id(),
        accounts: escrowq32026::accounts::Refund {
            maker: maker,
            mint_a: mint_a,
            maker_ata_a: maker_ata_a,
            escrow: escrow,
            vault: vault,
            token_program: TOKEN_PROGRAM_ID,
            system_program: SYSTEM_PROGRAM_ID,
        }
        .to_account_metas(None),
        data: escrowq32026::instruction::Refund {}.data(),
    };

    // Create and send the transaction containing the "Refund" instruction
    let message = Message::new(&[refund_ix], Some(&payer.pubkey()));
    let recent_blockhash = program.latest_blockhash();

    let transaction = Transaction::new(&[&payer], message, recent_blockhash);

    // Send the transaction and capture the result
    let tx = program.send_transaction(transaction).unwrap();

    // Log transaction details
    msg!("\n\nRefund transaction sucessful");
    msg!("CUs Consumed: {}", tx.compute_units_consumed);
    msg!("Tx Signature: {}", tx.signature);
    assert!(program.get_account(&escrow).is_none());
    assert!(program.get_account(&vault).is_none());
}

#[test]
fn test_take() {
    let (
        mut program,
        payer,
        maker,
        taker,
        mint_a,
        mint_b,
        maker_ata_a,
        maker_ata_b,
        taker_ata_a,
        taker_ata_b,
        escrow,
        vault,
    ) = setup();

    // First call Make to initialize the escrow so Take has something to work with
    let make_ix = Instruction {
        program_id: escrowq32026::id(),
        accounts: escrowq32026::accounts::Make {
            maker,
            mint_a,
            mint_b,
            maker_ata_a,
            escrow,
            vault,
            associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
            token_program: TOKEN_PROGRAM_ID,
            system_program: SYSTEM_PROGRAM_ID,
        }
        .to_account_metas(None),
        data: escrowq32026::instruction::Make {
            deposit: 10_000_000,
            seed: 123u64,
            receive: 10_000_000,
            expiration: 17780206209,
        }
        .data(),
    };

    let message = Message::new(&[make_ix], Some(&payer.pubkey()));
    let recent_blockhash = program.latest_blockhash();
    let transaction = Transaction::new(&[&payer], message, recent_blockhash);
    program.send_transaction(transaction).unwrap();

    // Mint token B to the taker so they can fulfill the escrow
    MintTo::new(&mut program, &payer, &mint_b, &taker_ata_b, 10_000_000)
        .send()
        .unwrap();

    let take_ix = Instruction {
        program_id: escrowq32026::id(),
        accounts: escrowq32026::accounts::Take {
            maker,
            taker: taker.pubkey(),
            mint_a,
            mint_b,
            maker_ata_b,
            taker_ata_a,
            taker_ata_b,
            escrow,
            vault,
            associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
            token_program: TOKEN_PROGRAM_ID,
            system_program: SYSTEM_PROGRAM_ID,
        }
        .to_account_metas(None),
        data: escrowq32026::instruction::Take {}.data(),
    };

    let message = Message::new(&[take_ix], Some(&taker.pubkey()));
    let recent_blockhash = program.latest_blockhash();

    let transaction = Transaction::new(&[&taker], message, recent_blockhash);

    let tx = program.send_transaction(transaction).unwrap();

    msg!("\n\nTake instruction successfull");
    msg!("CUs Consumed: {}", tx.compute_units_consumed);
    msg!("Tx Signature: {}", tx.signature);
    assert!(program.get_account(&escrow).is_none());
    assert!(program.get_account(&vault).is_none());
    assert_eq!(
        spl_token::state::Account::unpack(
            &program.get_account(&maker_ata_b).unwrap().data
        )
        .unwrap()
        .amount,
        10_000_000
    );
}

#[test]
fn test_update() {
    let (
        mut program,
        payer,
        maker,
        _taker,
        mint_a,
        mint_b,
        maker_ata_a,
        _maker_ata_b,
        _taker_ata_a,
        _taker_ata_b,
        escrow,
        vault,
    ) = setup();

    let initial_expiration: i64 = 17780206209;
    let seed = 123u64;

    // 1. Initialize Escrow with Make instruction
    let make_ix = Instruction {
        program_id: escrowq32026::id(),
        accounts: escrowq32026::accounts::Make {
            maker,
            mint_a,
            mint_b,
            maker_ata_a,
            escrow,
            vault,
            associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
            token_program: TOKEN_PROGRAM_ID,
            system_program: SYSTEM_PROGRAM_ID,
        }
        .to_account_metas(None),
        data: escrowq32026::instruction::Make {
            deposit: 10_000_000,
            seed,
            receive: 10_000_000,
            expiration: initial_expiration,
        }
        .data(),
    };

    let message = Message::new(&[make_ix], Some(&payer.pubkey()));
    let recent_blockhash = program.latest_blockhash();
    let transaction = Transaction::new(&[&payer], message, recent_blockhash);
    program.send_transaction(transaction).unwrap();

    // Verify initial escrow state
    let escrow_account = program.get_account(&escrow).unwrap();
    let escrow_data =
        escrowq32026::state::Escrow::try_deserialize(&mut escrow_account.data.as_ref()).unwrap();
    assert_eq!(escrow_data.expiration, initial_expiration);

    // 2. Test successful update: new expiration is further in the future
    let updated_expiration: i64 = 18880206209;
    let update_ix = Instruction {
        program_id: escrowq32026::id(),
        accounts: escrowq32026::accounts::Update {
            maker,
            escrow,
            clock: CLOCK_SYSVAR_ID,
        }
        .to_account_metas(None),
        data: escrowq32026::instruction::Update {
            expiration: updated_expiration,
        }
        .data(),
    };

    let message = Message::new(&[update_ix], Some(&payer.pubkey()));
    let recent_blockhash = program.latest_blockhash();
    let transaction = Transaction::new(&[&payer], message, recent_blockhash);
    let tx = program.send_transaction(transaction).unwrap();

    msg!("\n\nUpdate transaction successful");
    msg!("CUs Consumed: {}", tx.compute_units_consumed);
    msg!("Tx Signature: {}", tx.signature);

    // 3. Verify that only expiration changed; all other fields are untouched
    let escrow_account = program.get_account(&escrow).unwrap();
    let escrow_data =
        escrowq32026::state::Escrow::try_deserialize(&mut escrow_account.data.as_ref()).unwrap();
    assert_eq!(escrow_data.expiration, updated_expiration);
    assert_eq!(escrow_data.seed, seed);
    assert_eq!(escrow_data.maker, maker);
    assert_eq!(escrow_data.mint_a, mint_a);
    assert_eq!(escrow_data.mint_b, mint_b);
    assert_eq!(escrow_data.receive, 10_000_000);

    // 4. Test failure: new expiration is in the past → EscrowError::InvalidExpiration
    let invalid_expiration: i64 = -1;
    let invalid_update_ix = Instruction {
        program_id: escrowq32026::id(),
        accounts: escrowq32026::accounts::Update {
            maker,
            escrow,
            clock: CLOCK_SYSVAR_ID,
        }
        .to_account_metas(None),
        data: escrowq32026::instruction::Update {
            expiration: invalid_expiration,
        }
        .data(),
    };

    let message = Message::new(&[invalid_update_ix], Some(&payer.pubkey()));
    let recent_blockhash = program.latest_blockhash();
    let transaction = Transaction::new(&[&payer], message, recent_blockhash);
    let result = program.send_transaction(transaction);
    assert!(result.is_err(), "Expected update with past expiration to fail");
}
