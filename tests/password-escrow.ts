import * as anchor from "@coral-xyz/anchor";
import { Program, web3, BN, AnchorProvider } from "@coral-xyz/anchor";
import { splTokenProgram, SPL_TOKEN_PROGRAM_ID } from "@coral-xyz/spl-token";
import * as argon2 from "argon2";
import { deserialize } from "@phc/format";
import {
  PublicKey,
  Keypair,
  GetProgramAccountsFilter,
  MemcmpFilter,
} from "@solana/web3.js";

import { PasswordEscrow } from "../target/types/password_escrow";

import { expect } from "chai";
import * as chai from "chai";
import chaiAsPromised from "chai-as-promised";

import { createMintIfRequired, createToken, getATA, mintTo } from "./utils";

chai.use(chaiAsPromised);

describe("password-escrow", () => {
  // Configure the client to use the local cluster.
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.PasswordEscrow as Program<PasswordEscrow>;
  const splProgram = splTokenProgram({
    provider,
    programId: SPL_TOKEN_PROGRAM_ID,
  });

  const config = Keypair.generate();
  const mint = Keypair.generate();
  const funderToken = Keypair.generate();

  before(async () => {
    await createMintIfRequired(splProgram, mint, provider.wallet.publicKey);
    await createToken(
      splProgram,
      funderToken,
      mint.publicKey,
      provider.wallet.publicKey
    );
    await mintTo(
      splProgram,
      new BN(1_000_000_000),
      mint.publicKey,
      funderToken.publicKey,
      provider.wallet.publicKey
    );
  });

  it("Should initialize config", async () => {
    await program.methods
      // TODO: Add tests for config and withdraw
      .initializeConfig(
        provider.wallet.publicKey,
        provider.wallet.publicKey,
        new BN(1)
      )
      .accounts({
        payer: provider.wallet.publicKey,
        config: config.publicKey,
      })
      .signers([config])
      .rpc();
  });

  it("Should deposit to escrow", async () => {
    const password = "supersecretpassword";

    const digest = await argon2.hash(password, {
      associatedData: config.publicKey.toBuffer(),
    });
    const { salt, hash: seed } = deserialize(digest);
    const beneficiary_keypair = web3.Keypair.fromSeed(seed);
    const beneficiary = beneficiary_keypair.publicKey;

    const [escrow, _nonce] = PublicKey.findProgramAddressSync(
      [
        anchor.utils.bytes.utf8.encode("escrow"),
        beneficiary.toBuffer(),
        config.publicKey.toBuffer(),
      ],
      program.programId
    );
    const vault = getATA(escrow, mint.publicKey);

    await program.methods
      .deposit(new BN(1_000_000), salt, beneficiary)
      .accounts({
        config: config.publicKey,
        depositor: provider.wallet.publicKey,
        tokenAccount: funderToken.publicKey,
        mint: mint.publicKey,
        vault,
        escrow,
      })
      .rpc();
  });

  it("Should withdraw from escrow", async () => {
    const password = "supersecretpassword";
    let depositor = provider.wallet.publicKey;

    const accounts = await program.account.escrow.all([
      {
        memcmp: {
          offset: 8,
          bytes: anchor.utils.bytes.bs58.encode(
            Buffer.concat([config.publicKey.toBuffer(), depositor.toBuffer()])
          ),
        },
      },
    ]);
    expect(accounts.length).to.equal(1);

    const salt = Buffer.from(Uint8Array.from(accounts[0].account.salt));

    const seed = await argon2.hash(password, {
      associatedData: config.publicKey.toBuffer(),
      salt,
      raw: true,
    });

    const beneficiary = web3.Keypair.fromSeed(seed);
    expect(beneficiary.publicKey).to.deep.equal(
      accounts[0].account.beneficiary
    );

    await program.methods
      .withdraw()
      .accounts({
        config: config.publicKey,
        escrow: accounts[0].publicKey,
        beneficiary: beneficiary.publicKey,
        vault: accounts[0].account.vault,
        mint: mint.publicKey,
        destination: funderToken.publicKey,
      })
      .signers([beneficiary])
      .rpc();
  });
});
