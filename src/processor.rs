use crate::{
    instruction::TransferInstruction,
    state::{TransferInput, WithdrawInput, TransferData, InitTokenInput, WithdrawTokenInput,TransferToken},
};
use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    clock::Clock,
    entrypoint::ProgramResult,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::{rent::Rent, Sysvar},
    program::{invoke_signed, invoke},
    system_instruction, 
};

use super::error::{TokenError, EscrowError};
use spl_associated_token_account;

use crate::{PREFIX, PREFIX_TOKEN};
pub struct Processor;

impl Processor{
    pub fn process(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        instruction_data: &[u8]
    ) -> ProgramResult {
        let instruction = TransferInstruction::unpack(instruction_data)?;
        match instruction {
            TransferInstruction::CreateTranfer(data) => {
                Self::process_create_transfer(program_id, accounts, data)
            }
            TransferInstruction::Withdraw(data) => {
                Self::process_withdraw(program_id, accounts, data)
            }
            TransferInstruction::InitTokenTransfer(data) => {
                Self::process_create_token_transfer(program_id, accounts, data)
            }
            TransferInstruction::WithdrawToken(data) => {
                Self::process_withdraw_token(program_id, accounts, data)
            }
        }
    }

    fn process_create_transfer(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        data: TransferInput, 
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let escrow_account = next_account_info(account_info_iter)?;
        let sender_account = next_account_info(account_info_iter)?;
        let receiver_account = next_account_info(account_info_iter)?;
        let system_program = next_account_info(account_info_iter)?;        


        if !sender_account.is_signer {
            return Err(ProgramError::MissingRequiredSignature);
        }

        if *receiver_account.key != data.receiver {
            return Err(ProgramError::InvalidAccountData);
        }

        let (account_address, bump_seed) = Pubkey::find_program_address(
            &[
                PREFIX.as_bytes(),
                &sender_account.key.to_bytes()
                ],
            program_id,
        );

        if account_address != *escrow_account.key{
            return Err(ProgramError::InvalidAccountData);
        }

        invoke_signed(
            &system_instruction::create_account(
                sender_account.key, 
                escrow_account.key, 
                Rent::get()?.minimum_balance(std::mem::size_of::<TransferInput>()),
                std::mem::size_of::<TransferInput>().try_into().unwrap(),
                program_id
            ),
            &[sender_account.clone(), escrow_account.clone(),system_program.clone()],
            &[&[sender_account.key.as_ref(),&[bump_seed]]]
        )?;

        let escrow_data = TransferData::new(data, *sender_account.key);

        escrow_data.serialize(&mut &mut escrow_account.data.borrow_mut()[..])?;
        Ok(())
    }

    fn process_withdraw(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        data: WithdrawInput,
    ) -> ProgramResult{
        let account_info_iter = &mut accounts.iter();
        let escrow_account = next_account_info(account_info_iter)?; // pda storage
        let sender_account = next_account_info(account_info_iter)?; // sender account
        let receiver_account = next_account_info(account_info_iter)?; // receipent account
        let system_program = next_account_info(account_info_iter)?; // system program

        let escrow_data = TransferData::try_from_slice(&escrow_account.data.borrow()).expect("Failed to seriallize");

        if *receiver_account.key != escrow_data.receiver {
            return Err(ProgramError::IllegalOwner);
        }

        if !receiver_account.is_signer { 
            return Err(ProgramError::MissingRequiredSignature);
        }

        let (account_address, _bump_seed) = Pubkey::find_program_address(
            &[&sender_account.key.to_bytes()],
            program_id,
        );

        if account_address != *escrow_account.key{
            return Err(ProgramError::InvalidAccountData);
        }

        if escrow_data.start_time + (24*60*60) > Clock::get()?.unix_timestamp{ // 24 hours not passed yet
            return Err(EscrowError::WithdrawTimeLimitNotExceed.into());
        }
        
        let (_account_address, bump_seed) = Pubkey::find_program_address(
            &[&sender_account.key.to_bytes()],
            program_id,
        );
        let pda_signer_seeds: &[&[_]] = &[
            &sender_account.key.to_bytes(),
            &[bump_seed],
        ];

        invoke_signed(
            &system_instruction::transfer(
                sender_account.key,
                receiver_account.key,
                data.amount,
            ),
            &[
                sender_account.clone(),
                receiver_account.clone(),
                system_program.clone()
            ],
            &[pda_signer_seeds],
        )?;
        Ok(())
    }

    fn process_create_token_transfer(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        data: InitTokenInput,
    ) -> ProgramResult{
        let account_info_iter = &mut accounts.iter();
        let escrow_account = next_account_info(account_info_iter)?; // pda data storage
        let sender_account = next_account_info(account_info_iter)?; //sender
        let receiver_account = next_account_info(account_info_iter)?; // receiver
        let system_program = next_account_info(account_info_iter)?;  // system program
        let token_mint_info = next_account_info(account_info_iter)?; 
        let token_program_info = next_account_info(account_info_iter)?; 

        if token_program_info.key != &spl_token::id() {
            return Err(ProgramError::IncorrectProgramId);
        }    
        // Since we are performing system_instruction source account must be signer
        if !sender_account.is_signer {
            return Err(ProgramError::MissingRequiredSignature); 
        }

        let (account_address, bump_seed) = Pubkey::find_program_address(
            &[
                PREFIX_TOKEN.as_bytes(),
                &sender_account.key.to_bytes()
                ],
            program_id,
        );

        if account_address != *escrow_account.key{
            return Err(ProgramError::InvalidAccountData);
        }

        invoke_signed(
            &system_instruction::create_account(
                sender_account.key, 
                escrow_account.key, 
                Rent::get()?.minimum_balance(std::mem::size_of::<InitTokenInput>()),
                std::mem::size_of::<InitTokenInput>().try_into().unwrap(),
                program_id
            ),
            &[sender_account.clone(), escrow_account.clone(),system_program.clone()],
            &[&[sender_account.key.as_ref(),&[bump_seed]]]
        )?;

        let mut escrow = TransferToken::try_from_slice(&escrow_account.data.borrow())?;
        escrow.start_time = data.start_time;
        escrow.amount = data.amount;
        escrow.token_mint = *token_mint_info.key;
        escrow.sender = *sender_account.key;
        escrow.receiver = *receiver_account.key;

        escrow.serialize(&mut &mut escrow_account.data.borrow_mut()[..])?;

        Ok(())
    }

    fn process_withdraw_token(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        data : WithdrawTokenInput,
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let escrow_account = next_account_info(account_info_iter)?;
        let sender_account = next_account_info(account_info_iter)?;
        let receiver_account = next_account_info(account_info_iter)?;
        let system_program = next_account_info(account_info_iter)?; 
        let token_mint_info = next_account_info(account_info_iter)?;
        let token_program_info = next_account_info(account_info_iter)?; 
        let pda_associated_info = next_account_info(account_info_iter)?; // how/where is this account created
        let receiver_associated_info = next_account_info(account_info_iter)?;
        let rent_info = next_account_info(account_info_iter)?; 


        if *escrow_account.owner != *program_id {
            return Err(ProgramError::InvalidArgument);
        }

        if token_program_info.key != &spl_token::id() {
            return Err(ProgramError::IncorrectProgramId);
        }

        if !receiver_account.is_signer {
            return Err(ProgramError::MissingRequiredSignature); 
        }
        if escrow_account.data_is_empty(){
            return Err(ProgramError::UninitializedAccount);
        }

        let escrow = TransferToken::try_from_slice(&escrow_account.data.borrow())?;

        if escrow.token_mint != *token_mint_info.key {
            return Err(TokenError::PublicKeyMismatch.into());
        }

        if *receiver_account.key != escrow.receiver {
            return Err(TokenError::EscrowMismatch.into());
        }

        if escrow.start_time + (24*60*60) > Clock::get()?.unix_timestamp{ // 24 hours not passed yet
            return Err(EscrowError::WithdrawTimeLimitNotExceed.into());
        }

        //creating associated token program for receiver to transfer token
        invoke(
            &spl_associated_token_account::create_associated_token_account(
                receiver_account.key,
                receiver_account.key,
                token_mint_info.key
            ), 
            &[
                receiver_account.clone(),
                receiver_associated_info.clone(),
                receiver_account.clone(),
                token_mint_info.clone(),
                system_program.clone(),
                token_program_info.clone(),
                rent_info.clone()
            ]
        )?;

        let (_account_address, bump) = Pubkey::find_program_address(
            &[&sender_account.key.to_bytes()], 
            program_id
        );

        let pda_signer_seeds: &[&[_]] = &[&sender_account.key.to_bytes(), &[bump]];
        
        //transfering token to receiver_associated_info
        invoke_signed(
            &spl_token::instruction::transfer(
                token_program_info.key,
                pda_associated_info.key,
                receiver_associated_info.key,
                escrow_account.key,
                &[escrow_account.key],
                data.amount,
            )?,
            &[
                token_program_info.clone(),
                pda_associated_info.clone(),
                receiver_associated_info.clone(),
                escrow_account.clone(),
                system_program.clone()
            ],&[&pda_signer_seeds],
        )?;

        Ok(())

    }
}