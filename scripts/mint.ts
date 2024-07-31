// Migrations are an early feature. Currently, they're nothing more than this
// single deploy script that's invoked from the CLI, injecting a provider
// configured from the workspace's Anchor.toml.
import { exit } from "process"
import web3, { clusterApiUrl, Connection, Keypair } from "@solana/web3.js"
import NodeWallet from '@project-serum/anchor/dist/cjs/nodewallet'
import { 
  TOKEN_PROGRAM_ID,
  createMint,
  getOrCreateAssociatedTokenAccount,
  mintTo,
  AccountLayout
} from "@solana/spl-token"

async function main () {
  const connection = new Connection("http://127.0.0.1:8899")
  const secretKeyArray = [] // Uint8Array - secret key
  const secretKey = Uint8Array.from(secretKeyArray)

  const wallet = new NodeWallet(Keypair.fromSecretKey(secretKey))

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

  const tokenAccount = await getOrCreateAssociatedTokenAccount(
    connection,
    wallet.payer,
    mint,
    wallet.publicKey
  )

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
  console.log("Balance:", balance)
};

const getBalance  = async (key: web3.PublicKey, mint: web3.PublicKey, connection: web3.Connection) => {
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