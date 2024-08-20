// Migrations are an early feature. Currently, they're nothing more than this
// single deploy script that's invoked from the CLI, injecting a provider
// configured from the workspace's Anchor.toml.
import { exit } from "process"
import web3, { clusterApiUrl, Connection, Keypair, sendAndConfirmTransaction } from "@solana/web3.js"
import NodeWallet from '@project-serum/anchor/dist/cjs/nodewallet'
import {
    TOKEN_PROGRAM_ID,
    createMint,
    getOrCreateAssociatedTokenAccount,
    mintTo,
    AccountLayout,
    approve,
    createApproveInstruction
} from "@solana/spl-token"
import { Program, Idl, AnchorProvider, BN } from "@project-serum/anchor"

import idl from '../target/idl/bluescrypto_staking.json'
// import { Provider } from "@project-serum/common"

async function main() {
    const connection = new Connection("http://127.0.0.1:8899", { commitment: "confirmed" })
    // const connection = new Connection("https://api.devnet.solana.com", { commitment: "confirmed" })

    const secretKeyArray = [222, 147, 169, 34, 116, 179, 200, 5, 86, 140, 181, 225, 31, 14, 63, 93, 180, 62, 120, 225, 122, 9, 190, 99, 129, 85, 40, 80, 91, 222, 99, 7, 121, 240, 67, 114, 117, 90, 240, 61, 220, 246, 105, 199, 173, 117, 102, 49, 124, 208, 12, 248, 196, 17, 159, 114, 132, 76, 126, 76, 92, 61, 95, 67]
    const secretKey = Uint8Array.from(secretKeyArray)

    const wallet = new NodeWallet(Keypair.fromSecretKey(secretKey))
    const provider = new AnchorProvider(connection, wallet, { commitment: "confirmed" })

    const stakingProgramId = new web3.PublicKey("2XzGonB3VWc7KnGdUdU5TM7sH9kSr5PueVLc6isXTSDD")
    const stakingProgram = new Program(idl as Idl, stakingProgramId, provider)

    const mint = await createMint(
        connection,
        wallet.payer,
        wallet.publicKey,
        null,
        9,
        undefined,
        {},
        TOKEN_PROGRAM_ID
    )

    const [stakingStorage, stakingStorageBump] = web3.PublicKey.findProgramAddressSync([], stakingProgramId)
    const [escrowVault, escroVaultBump] = web3.PublicKey.findProgramAddressSync([
        Buffer.from("escrow_vault"),
        mint.toBuffer()
    ], stakingProgramId)

    const tokenAccount = await getOrCreateAssociatedTokenAccount(
        connection,
        wallet.payer,
        mint,
        wallet.publicKey
    )

    const stakingProgramTokenAccount = await getOrCreateAssociatedTokenAccount(
        connection,
        wallet.payer,
        mint,
        stakingProgramId
    )

    // mint spl token to owner wallet
    await mintTo(
        connection,
        wallet.payer,
        mint,
        tokenAccount.address,
        wallet.publicKey,
        10000000000000
    )

    const balance = await getBalance(wallet.publicKey, mint, connection)

    console.log("Mint:", mint.toString())
    console.log("Wallet:", wallet.publicKey.toString())
    console.log("Wallet ATA:", tokenAccount.address.toString())
    console.log("Balance:", balance)
    console.log("Escrow:", escrowVault.toString())
    // console.log("Escrow ATA:", escrowVaultTokenAccount.address.toString())
    console.log("Staking program ATA:", stakingProgramTokenAccount.address.toString())

    // initialize staking program
    await stakingProgram.methods.initialize().accounts({
        stakingStorage,
        escrowVault,
        mint
    }).signers([wallet.payer]).rpc()

    console.log("initialized")

    // await stakingProgram.methods.initialize().accounts({
    //     stakingStorage,
    //     escrowVault,
    //     mint,
    //     signer: wallet.payer.publicKey,
    //     systemProgram: web3.SystemProgram.programId,
    //     tokenProgram: TOKEN_PROGRAM_ID,
    // }).signers([wallet.payer]).rpc();

    await approve(
        connection,
        wallet.payer,
        tokenAccount.address,
        escrowVault,
        wallet.payer,
        1000000
    )

    console.log("token approved")

    const escrowBalanceBeforeCharge = await getBalance(escrowVault, mint, connection)
    console.log("escrow balance before charge:", escrowBalanceBeforeCharge)

    await stakingProgram.methods.chargeEscrow(new BN(1000000)).accounts({
        from: tokenAccount.address,
        authority: wallet.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
        escrowVault,
        mint
    }).rpc()

    console.log("escrow charged")
    const escrowBalanceAfterCharge = await getBalance(escrowVault, mint, connection)
    console.log("escrow balance afater charge:", escrowBalanceAfterCharge)
};

const getBalance = async (key: web3.PublicKey, mint: web3.PublicKey, connection: web3.Connection) => {
    const token_account = await connection.getTokenAccountsByOwner(
        key,
        {
            mint
        }
    )

    const balance = token_account.value.length > 0 ? AccountLayout.decode(token_account.value[0].account.data).amount : 0

    return balance
}

main().catch(err => {
    console.error(err)

    exit(1)
})