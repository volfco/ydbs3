# prototype: ydbs3 
S3 on top of YDB. A Prototype

## iteration 0
**Concept** Objects are split up into blocks, which are stored as rows in the block table.

**Goals**
1. Basic Storage of Files in YDB. Doesn't need to be fast or efficient

### Creation
The file is split into as many even blocks as possible while keeping each block under 8MB. Each block is written to the
blocks table. Once completed, the metadata is written to the object table.

8MB is a recommendation from YDB, so let's stick to that for now

### Retrieval
The object is retrieved by reading the metadata from the object table. The topology field is read to get blocks to load,
and the blocks are loaded from the blocks table in parallel. Once the first block is retrieved, it is sent to the client.

### Updating
The object configuration is read to figure out the block size. Updated object is chunked according to that block size. 

Each chunk is compared to the existing chunk. If it is different, then the different chunk is written to the block table 
with an incremented blk_id. Then the topology field is updated to point to the new block.

... then the old block is deleted i guess.

... yeah, so the topology field is needed because what if a new chunk of data is inserted in the middle of the file. We could
then only insert the new blocks, and re-order the topology field. 

and for iteration 0 revision 1, blocks could lose all reference to file and just be blk_id. Then topology would just reference
a list of blk_id, and then blocks could also be deduplicated.
    ... for deduplication there could be a "short hash" column, that is kinda like a bloom filter. It's easy to calculate on the client side
    and queried against the database. The result would be "very close" rows that more likely than not would fully match/hash the given block. 
        ... this seems like a bloom filter. YDB makes mention of supporting bloom filters internally, so maybe this could be very easy to do


```sql
create table block (
    file_id     Utf8,     -- File UUID
    blk_id      Uint64,     -- Block ID

    length      Uint64,     -- Block Length in bytes
    data        String,      -- Block Raw Data
    PRIMARY KEY (file_id, blk_id)
);

create table object (
    file_id     Utf8,         -- File ID

    path        Utf8,         -- User Friendly File Path
    mime_type   Utf8,         -- Mime Type
    blk_size    Uint64,         -- Block Size
    topology    Yson,   -- List of Block IDs that make up this file, in order

    PRIMARY KEY (file_id)
);
```

YDB based SQS??