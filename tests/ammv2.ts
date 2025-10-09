import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { AmmV2 } from "../target/types/amm_v2";
import { assert } from "chai";
import {
  createMint,
  createAssociatedTokenAccount,
  mintTo,
  getAccount,
  TOKEN_PROGRAM_ID,
  ASSOCIATED_TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
import {
  Keypair,
  PublicKey,
  SystemProgram,
  LAMPORTS_PER_SOL,
} from "@solana/web3.js";

interface PoolAccounts {
  authority: Keypair;
  payer: Keypair;
  tokenAMint: PublicKey;
  tokenBMint: PublicKey;
  tokenAVault: PublicKey;
  tokenBVault: PublicKey;
  lpTokenMint: PublicKey;
  liquidityPool: PublicKey;
  poolAuthority: PublicKey;
}

interface LiquidityProvider {
  signer: Keypair;
  tokenAAccount: PublicKey;
  tokenBAccount: PublicKey;
  lpTokenAccount: PublicKey;
}

describe("AMM V2 Tests", () => {
  // Configure the client to use the local cluster
  anchor.setProvider(anchor.AnchorProvider.env());
  const program = anchor.workspace.AmmV2 as Program<AmmV2>;
  const provider = anchor.getProvider() as anchor.AnchorProvider;
  const connection = provider.connection;

  let pool: PoolAccounts;
  const TOKEN_DECIMALS = 9;

  /**
   * Helper function to get token balance as a number
   */
  async function getTokenBalance(tokenAccount: PublicKey): Promise<number> {
    const account = await getAccount(connection, tokenAccount);
    return Number(account.amount) / Math.pow(10, TOKEN_DECIMALS);
  }

  /**
   * Helper function to convert amount to proper token units
   */
  function toTokenAmount(amount: number): anchor.BN {
    return new anchor.BN(amount * Math.pow(10, TOKEN_DECIMALS));
  }

  /**
   * Setup a new liquidity provider with token accounts and initial balances
   */
  async function setupLiquidityProvider(
    userPublicKey: PublicKey,
    initialAmount: number
  ): Promise<[PublicKey, PublicKey, PublicKey]> {
    // Create associated token accounts for both tokens
    const tokenAAccount = await createAssociatedTokenAccount(
      connection,
      pool.payer,
      pool.tokenAMint,
      userPublicKey
    );

    const tokenBAccount = await createAssociatedTokenAccount(
      connection,
      pool.payer,
      pool.tokenBMint,
      userPublicKey
    );

    // Create LP token account
    const lpTokenAccount = await createAssociatedTokenAccount(
      connection,
      pool.payer,
      pool.lpTokenMint,
      userPublicKey
    );

    // Mint initial tokens to user
    const mintAmount = initialAmount * Math.pow(10, TOKEN_DECIMALS);

    await mintTo(
      connection,
      pool.payer,
      pool.tokenAMint,
      tokenAAccount,
      pool.authority,
      mintAmount
    );

    await mintTo(
      connection,
      pool.payer,
      pool.tokenBMint,
      tokenBAccount,
      pool.authority,
      mintAmount
    );

    return [tokenAAccount, tokenBAccount, lpTokenAccount];
  }

  it("Initializes a new liquidity pool", async () => {
    // Generate authority and request airdrop
    const authority = Keypair.generate();
    const airdropSig = await connection.requestAirdrop(
      authority.publicKey,
      100 * LAMPORTS_PER_SOL
    );
    await connection.confirmTransaction(airdropSig);

    // Create token mints
    const tokenAMint = await createMint(
      connection,
      authority,
      authority.publicKey,
      authority.publicKey,
      TOKEN_DECIMALS
    );

    const tokenBMint = await createMint(
      connection,
      authority,
      authority.publicKey,
      authority.publicKey,
      TOKEN_DECIMALS
    );

    // Derive PDA accounts
    const [liquidityPool] = PublicKey.findProgramAddressSync(
      [
        Buffer.from("liquidity_pool"),
        tokenAMint.toBuffer(),
        tokenBMint.toBuffer(),
      ],
      program.programId
    );

    const [poolAuthority] = PublicKey.findProgramAddressSync(
      [Buffer.from("pool_authority"), liquidityPool.toBuffer()],
      program.programId
    );

    const [tokenAVault] = PublicKey.findProgramAddressSync(
      [Buffer.from("token_a_vault"), liquidityPool.toBuffer()],
      program.programId
    );

    const [tokenBVault] = PublicKey.findProgramAddressSync(
      [Buffer.from("token_b_vault"), liquidityPool.toBuffer()],
      program.programId
    );

    const [lpTokenMint] = PublicKey.findProgramAddressSync(
      [Buffer.from("lp_token_mint"), liquidityPool.toBuffer()],
      program.programId
    );

    // Initialize pool with 0.01% fee (1/10000)
    const feeNumerator = new anchor.BN(1);
    const feeDenominator = new anchor.BN(10000);

    await program.methods
      .initializePool(feeNumerator, feeDenominator)
      .accounts({
        tokenAMint: tokenAMint,
        tokenBMint: tokenBMint,
        liquidityPool: liquidityPool,
        poolAuthority: poolAuthority,
        tokenAVault: tokenAVault,
        tokenBVault: tokenBVault,
        lpTokenMint: lpTokenMint,
        payer: provider.wallet.publicKey,
        systemProgram: SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
      })
      .rpc();

    // Store pool info for other tests
    pool = {
      authority: authority,
      payer: authority,
      tokenAMint: tokenAMint,
      tokenBMint: tokenBMint,
      tokenAVault: tokenAVault,
      tokenBVault: tokenBVault,
      lpTokenMint: lpTokenMint,
      liquidityPool: liquidityPool,
      poolAuthority: poolAuthority,
    };

    console.log("Pool initialized successfully");
  });

  let liquidityProvider1: LiquidityProvider;

  it("Adds initial liquidity to the pool", async () => {
    const lpSigner = Keypair.generate();
    const [tokenAAccount, tokenBAccount, lpTokenAccount] =
      await setupLiquidityProvider(lpSigner.publicKey, 100);

    liquidityProvider1 = {
      signer: lpSigner,
      tokenAAccount: tokenAAccount,
      tokenBAccount: tokenBAccount,
      lpTokenAccount: lpTokenAccount,
    };

    const depositAmountA = toTokenAmount(50);
    const depositAmountB = toTokenAmount(50);

    await program.methods
      .depositLiquidity(depositAmountA, depositAmountB)
      .accounts({
        liquidityPool: pool.liquidityPool,
        poolAuthority: pool.poolAuthority,
        tokenAVault: pool.tokenAVault,
        tokenBVault: pool.tokenBVault,
        lpTokenMint: pool.lpTokenMint,
        userTokenAAccount: tokenAAccount,
        userTokenBAccount: tokenBAccount,
        userLpTokenAccount: lpTokenAccount,
        user: lpSigner.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .signers([lpSigner])
      .rpc();

    // Verify balances
    const lpTokenBalance = await getTokenBalance(lpTokenAccount);
    const poolState = await program.account.liquidityPool.fetch(
      pool.liquidityPool
    );
    const totalLpIssued = poolState.totalLpTokensIssued.toNumber();

    console.log("LP tokens received:", lpTokenBalance);
    console.log(
      "Total LP tokens issued:",
      totalLpIssued / Math.pow(10, TOKEN_DECIMALS)
    );

    assert(lpTokenBalance > 0, "LP tokens should be minted");

    // Verify vault balances
    const vaultABalance = await getTokenBalance(pool.tokenAVault);
    const vaultBBalance = await getTokenBalance(pool.tokenBVault);

    console.log("Vault A balance:", vaultABalance);
    console.log("Vault B balance:", vaultBBalance);

    assert(vaultABalance === 50, "Vault A should have 50 tokens");
    assert(vaultBBalance === 50, "Vault B should have 50 tokens");

    console.log("Initial liquidity added successfully");
  });

  let liquidityProvider2: LiquidityProvider;

  it("Adds second liquidity deposit to the pool", async () => {
    const lpSigner = Keypair.generate();
    const [tokenAAccount, tokenBAccount, lpTokenAccount] =
      await setupLiquidityProvider(lpSigner.publicKey, 100);

    liquidityProvider2 = {
      signer: lpSigner,
      tokenAAccount: tokenAAccount,
      tokenBAccount: tokenBAccount,
      lpTokenAccount: lpTokenAccount,
    };

    const depositAmountA = toTokenAmount(50);
    const depositAmountB = toTokenAmount(50);

    await program.methods
      .depositLiquidity(depositAmountA, depositAmountB)
      .accounts({
        liquidityPool: pool.liquidityPool,
        poolAuthority: pool.poolAuthority,
        tokenAVault: pool.tokenAVault,
        tokenBVault: pool.tokenBVault,
        lpTokenMint: pool.lpTokenMint,
        userTokenAAccount: tokenAAccount,
        userTokenBAccount: tokenBAccount,
        userLpTokenAccount: lpTokenAccount,
        user: lpSigner.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .signers([lpSigner])
      .rpc();

    const lpTokenBalance = await getTokenBalance(lpTokenAccount);
    const poolState = await program.account.liquidityPool.fetch(
      pool.liquidityPool
    );
    const totalLpIssued = poolState.totalLpTokensIssued.toNumber();

    console.log("LP2 tokens received:", lpTokenBalance);
    console.log(
      "Total LP tokens issued:",
      totalLpIssued / Math.pow(10, TOKEN_DECIMALS)
    );

    assert(lpTokenBalance > 0, "LP tokens should be minted");

    const vaultABalance = await getTokenBalance(pool.tokenAVault);
    const vaultBBalance = await getTokenBalance(pool.tokenBVault);

    console.log("Vault A balance:", vaultABalance);
    console.log("Vault B balance:", vaultBBalance);

    assert(vaultABalance === 100, "Vault A should have 100 tokens");
    assert(vaultBBalance === 100, "Vault B should have 100 tokens");

    console.log("Second liquidity deposit successful");
  });

  it("Adds third liquidity deposit with larger Token B amount", async () => {
    const lpSigner = Keypair.generate();
    const [tokenAAccount, tokenBAccount, lpTokenAccount] =
      await setupLiquidityProvider(lpSigner.publicKey, 100);

    const depositAmountA = toTokenAmount(25);
    const depositAmountB = toTokenAmount(100); // More than needed

    await program.methods
      .depositLiquidity(depositAmountA, depositAmountB)
      .accounts({
        liquidityPool: pool.liquidityPool,
        poolAuthority: pool.poolAuthority,
        tokenAVault: pool.tokenAVault,
        tokenBVault: pool.tokenBVault,
        lpTokenMint: pool.lpTokenMint,
        userTokenAAccount: tokenAAccount,
        userTokenBAccount: tokenBAccount,
        userLpTokenAccount: lpTokenAccount,
        user: lpSigner.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .signers([lpSigner])
      .rpc();

    const lpTokenBalance = await getTokenBalance(lpTokenAccount);
    console.log("LP3 tokens received:", lpTokenBalance);

    const vaultABalance = await getTokenBalance(pool.tokenAVault);
    const vaultBBalance = await getTokenBalance(pool.tokenBVault);

    console.log("Vault A balance:", vaultABalance);
    console.log("Vault B balance:", vaultBBalance);

    assert(vaultABalance === 125, "Vault A should have 125 tokens");
    assert(vaultBBalance === 125, "Vault B should have 125 tokens");

    console.log(
      "Third liquidity deposit successful (pool maintained 1:1 ratio)"
    );
  });

  it("Removes liquidity from the pool", async () => {
    const beforeTokenA = await getTokenBalance(
      liquidityProvider1.tokenAAccount
    );
    const beforeTokenB = await getTokenBalance(
      liquidityProvider1.tokenBAccount
    );
    const beforeLpTokens = await getTokenBalance(
      liquidityProvider1.lpTokenAccount
    );

    console.log("Before withdrawal - Token A:", beforeTokenA);
    console.log("Before withdrawal - Token B:", beforeTokenB);
    console.log("Before withdrawal - LP tokens:", beforeLpTokens);

    const burnAmount = toTokenAmount(50);

    await program.methods
      .withdrawLiquidity(burnAmount)
      .accounts({
        liquidityPool: pool.liquidityPool,
        poolAuthority: pool.poolAuthority,
        tokenAVault: pool.tokenAVault,
        tokenBVault: pool.tokenBVault,
        lpTokenMint: pool.lpTokenMint,
        userTokenAAccount: liquidityProvider1.tokenAAccount,
        userTokenBAccount: liquidityProvider1.tokenBAccount,
        userLpTokenAccount: liquidityProvider1.lpTokenAccount,
        user: liquidityProvider1.signer.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .signers([liquidityProvider1.signer])
      .rpc();

    const afterTokenA = await getTokenBalance(liquidityProvider1.tokenAAccount);
    const afterTokenB = await getTokenBalance(liquidityProvider1.tokenBAccount);
    const afterLpTokens = await getTokenBalance(
      liquidityProvider1.lpTokenAccount
    );

    console.log("After withdrawal - Token A:", afterTokenA);
    console.log("After withdrawal - Token B:", afterTokenB);
    console.log("After withdrawal - LP tokens:", afterLpTokens);

    assert(afterLpTokens < beforeLpTokens, "LP tokens should decrease");
    assert(afterTokenA > beforeTokenA, "Token A balance should increase");
    assert(afterTokenB > beforeTokenB, "Token B balance should increase");

    const vaultABalance = await getTokenBalance(pool.tokenAVault);
    const vaultBBalance = await getTokenBalance(pool.tokenBVault);

    console.log("Vault A balance:", vaultABalance);
    console.log("Vault B balance:", vaultBBalance);

    console.log("Liquidity removed successfully");
  });

  it("Performs a token swap", async () => {
    const swapper = Keypair.generate();

    // Create token accounts for swapper
    const tokenAAccount = await createAssociatedTokenAccount(
      connection,
      pool.payer,
      pool.tokenAMint,
      swapper.publicKey
    );

    const tokenBAccount = await createAssociatedTokenAccount(
      connection,
      pool.payer,
      pool.tokenBMint,
      swapper.publicKey
    );

    // Mint tokens to swapper
    const initialAmount = 100;
    await mintTo(
      connection,
      pool.payer,
      pool.tokenAMint,
      tokenAAccount,
      pool.authority,
      initialAmount * Math.pow(10, TOKEN_DECIMALS)
    );

    const beforeTokenA = await getTokenBalance(tokenAAccount);
    const beforeTokenB = await getTokenBalance(tokenBAccount);

    console.log("Before swap - Token A:", beforeTokenA);
    console.log("Before swap - Token B:", beforeTokenB);

    // Swap 10 Token A for Token B
    const swapAmount = toTokenAmount(10);
    const minOutputAmount = new anchor.BN(0);

    await program.methods
      .swapTokens(swapAmount, minOutputAmount)
      .accounts({
        liquidityPool: pool.liquidityPool,
        poolAuthority: pool.poolAuthority,
        inputTokenVault: pool.tokenAVault,
        outputTokenVault: pool.tokenBVault,
        userInputTokenAccount: tokenAAccount,
        userOutputTokenAccount: tokenBAccount,
        user: swapper.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .signers([swapper])
      .rpc();

    const afterTokenA = await getTokenBalance(tokenAAccount);
    const afterTokenB = await getTokenBalance(tokenBAccount);

    console.log("After swap - Token A:", afterTokenA);
    console.log("After swap - Token B:", afterTokenB);

    assert(afterTokenA < beforeTokenA, "Token A should decrease");
    assert(afterTokenB > beforeTokenB, "Token B should increase");

    console.log("Swap executed successfully");
  });

  it("Removes liquidity after swap (with profit from fees)", async () => {
    const beforeTokenA = await getTokenBalance(
      liquidityProvider2.tokenAAccount
    );
    const beforeTokenB = await getTokenBalance(
      liquidityProvider2.tokenBAccount
    );
    const beforeLpTokens = await getTokenBalance(
      liquidityProvider2.lpTokenAccount
    );

    console.log("Before withdrawal - Token A:", beforeTokenA);
    console.log("Before withdrawal - Token B:", beforeTokenB);
    console.log("Before withdrawal - LP tokens:", beforeLpTokens);

    const burnAmount = toTokenAmount(50);

    await program.methods
      .withdrawLiquidity(burnAmount)
      .accounts({
        liquidityPool: pool.liquidityPool,
        poolAuthority: pool.poolAuthority,
        tokenAVault: pool.tokenAVault,
        tokenBVault: pool.tokenBVault,
        lpTokenMint: pool.lpTokenMint,
        userTokenAAccount: liquidityProvider2.tokenAAccount,
        userTokenBAccount: liquidityProvider2.tokenBAccount,
        userLpTokenAccount: liquidityProvider2.lpTokenAccount,
        user: liquidityProvider2.signer.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .signers([liquidityProvider2.signer])
      .rpc();

    const afterTokenA = await getTokenBalance(liquidityProvider2.tokenAAccount);
    const afterTokenB = await getTokenBalance(liquidityProvider2.tokenBAccount);
    const afterLpTokens = await getTokenBalance(
      liquidityProvider2.lpTokenAccount
    );

    console.log("After withdrawal - Token A:", afterTokenA);
    console.log("After withdrawal - Token B:", afterTokenB);
    console.log("After withdrawal - LP tokens:", afterLpTokens);

    assert(afterLpTokens < beforeLpTokens, "LP tokens should decrease");
    assert(afterTokenA > beforeTokenA, "Token A balance should increase");
    assert(afterTokenB > beforeTokenB, "Token B balance should increase");

    // Check for profit from swap fees
    const tokenAGain = afterTokenA - beforeTokenA;
    const tokenBGain = afterTokenB - beforeTokenB;

    console.log("Token A gained:", tokenAGain);
    console.log("Token B gained:", tokenBGain);

    // Should have earned more Token A (due to swap fee) and less Token B (impermanent loss)
    assert(tokenAGain > 50, "Should earn profit in Token A from swap fees");
    assert(tokenBGain < 50, "Should experience impermanent loss in Token B");

    console.log(
      "Liquidity removed with profit from fees (impermanent loss visible)"
    );
  });
});
