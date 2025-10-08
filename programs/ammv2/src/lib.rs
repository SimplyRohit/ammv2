use anchor_lang::prelude::*;

declare_id!("EkN4yqcURunz6Xj19tgsJQLmwqZoGEZmWFJwajCo2Tg8");

#[program]
pub mod ammv2 {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        msg!("Greetings from: {:?}", ctx.program_id);
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize {}
