use ydb::{ydb_params, TableClient, Bytes};
use anyhow::Result;
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use tracing::{info, warn};


const TARGET_BLOCK_SIZES_KB: [u64; 4] = [512, 1024, 2048, 4096];


struct Object {
    file_id: String,
    path: String,
    blk_size: u64,
    topology: Topology
}

struct Topology {
    blocks: Vec<String>
}

struct Block {
    blk_id: String,
    length: u64,

}

pub async fn init_db() -> ydb::YdbResult<ydb::Client> {
    let conn_string = std::env::var("YDB_CONNECTION_STRING")
        .unwrap_or_else(|_| "grpc://localhost:2136?database=/local".to_string());
    let client = ydb::ClientBuilder::new_from_connection_string(conn_string)?.client()?;

    client.wait().await?;

    Ok(client)
}

pub async fn write_row(table_client: TableClient, file_id: String, block_id: u64, length: u64, data: Vec<u8>) {
    info!("Writing row for file_id: {}, block_id: {}, length: {}", file_id, block_id, length);
    let data: Bytes = Bytes::from(data);
    table_client.retry_transaction(|tx| async {
        let mut tx = tx;
        tx.query(
            ydb::Query::new(
                "DECLARE $file_id AS Utf8;
                DECLARE $block_id AS Uint64;
                DECLARE $length AS Uint64;
                DECLARE $data AS String;
                REPLACE INTO block (file_id, blk_id, length, data) VALUES ($file_id, $block_id, $length, $data);
                "
            )
            .with_params(ydb_params!("$file_id" => file_id.clone(), "$block_id" => block_id, "$length" => length, "$data" => data.clone()))
        ).await?;
        tx.commit().await?;
        Ok(())
    }).await.unwrap();
    info!("Wrote row for file_id: {}, block_id: {}, length: {}", file_id, block_id, length);
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let db = init_db().await.unwrap();

    // read the file
    let mut file = File::open("Cybermutt_alt.jpg").await.unwrap();

    let mut contents = vec![];
    file.read_to_end(&mut contents).await.unwrap();

    let len = contents.len();

    // pick the optimal block size from TARGET_BLOCK_SIZES_KB for `len` bytes
    let block_size = TARGET_BLOCK_SIZES_KB
        .iter()
        .copied()
        .min_by_key(|&size| (size * 1024 - len as u64 % (size * 1024)) % (size * 1024))
        .unwrap();

    info!("selected object block size: {} KB", block_size);

    // chunk the file into blocks using block_size
    let blocks = contents
        .chunks(block_size as usize * 1024)
        .map(|chunk| chunk.to_vec())
        .collect::<Vec<Vec<u8>>>();

    info!("resulting object chunks: {}", blocks.len());

    let uuid = uuid::Uuid::new_v4().to_string();
    let path = "Cybermutt_alt.jpg".to_string();

    let table_client = db.table_client();

    // write each row
    let mut topology = vec![];
    for (i, block) in blocks.iter().enumerate() {
        tokio::spawn(write_row(table_client.clone(), uuid.clone(), i as u64, block.len() as u64, block.clone()));
        topology.push(i);
    }

    // write the object metadata
    table_client.retry_transaction(|tx| async {
        let mut tx = tx;
        warn!("topology: {:?}", &topology);
        tx.query(
            ydb::Query::from(
                "DECLARE $uuid AS Utf8;
                DECLARE $path AS Utf8;
                REPLACE INTO object (file_id, path) VALUES ($uuid, $path);
                "
            )
            .with_params(ydb_params!("$uuid" => uuid.clone(), "$path" => path.clone()))
        ).await?;
        tx.commit().await?;
        Ok(())
    }).await.unwrap();
}