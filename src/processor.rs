use crate::{
    instruction::TransferInstruction,
    state::{TransferInput, WithdrawInput, TransferData},
};
use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    clock::Clock,
    entrypoint::ProgramResult,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::{rent::Rent, Sysvar},
    program::{invoke_signed},
    system_instruction,
};
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
            &[&sender_account.key.to_bytes()],
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
        let escrow_account = next_account_info(account_info_iter)?;
        let sender_account = next_account_info(account_info_iter)?;
        let receiver_account = next_account_info(account_info_iter)?;
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
            return Err(ProgramError::Custom(999)) 
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
}