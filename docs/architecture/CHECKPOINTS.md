# Atomic checkpoints

Required write pattern:

```text
serialize checkpoint without checksum
compute checksum
write temporary file
fsync temporary file
fsync parent directory
rename temporary -> latest checkpoint
fsync parent directory
```

Checkpoint content includes:

- run id
- persistent state
- completed components
- component states
- search state
- partial result index
- ranking state
- RNG state
- metadata
- checksum
