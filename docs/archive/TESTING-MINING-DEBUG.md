# Testing Client-Side Mining with Debug Logging

## Step 1: Enable Debug Logging

Open browser console and run:
```javascript
localStorage.setItem('coinjecture:mining-debug', 'true');
```

Or in the browser console before mining:
```javascript
// Enable debug logging
localStorage.setItem('coinjecture:mining-debug', 'true');
console.log('✅ Debug logging enabled');
```

## Step 2: Mine a Block

1. Go to the terminal page: https://d1f2zzpbyxllz7.cloudfront.net/terminal
2. Run: `mine submit`
3. Watch the browser console for debug output

## Step 3: Check Server Logs

SSH into the droplet and check logs:
```bash
ssh -i ~/.ssh/COINjecture-Key root@143.110.139.166
docker logs coinject-node 2>&1 | grep -E '🔍|📄|Difficulty check|Header JSON' | tail -30
```

## Step 4: Compare JSON Payloads

### Client Output (Browser Console)
Look for:
- `🧠 Client header JSON (hashed payload):` - The exact JSON string
- `🔍 Client header hash calculation:` - JSON bytes and hash details
- `🔍 Client header object (before JSON.stringify):` - The object structure

### Server Output (Droplet Logs)
Look for:
- `🔍 Difficulty check (JSON):` - Hash and leading zeros
- `📄 Header JSON (server hashed payload, X bytes):` - The exact JSON string
- `📄 Header JSON bytes (first 200):` - The byte array representation

## Step 5: Identify Differences

Compare:
1. **JSON String**: Exact character-by-character comparison
2. **Field Order**: Ensure fields are in the same order
3. **Float Precision**: Check if floats are serialized differently (e.g., `1.0` vs `1`)
4. **Array Formatting**: Check if arrays have spaces or different formatting
5. **Byte Array**: Compare the first 200 bytes from both sides

## Common Issues to Check

1. **Float Serialization**: 
   - Client: `1.4142135623730951` (JavaScript default)
   - Server: `1.4142135623730951` (Rust serde_json default)
   - May differ in precision or trailing zeros

2. **Field Order**:
   - Ensure fields match Rust struct order exactly
   - Rust struct order: version, height, prev_hash, timestamp, transactions_root, solutions_root, commitment, work_score, miner, nonce, solve_time_us, verify_time_us, time_asymmetry_ratio, solution_quality, complexity_weight, energy_estimate_joules

3. **Array Serialization**:
   - Client: `[0,1,2,3]` (no spaces)
   - Server: `[0,1,2,3]` (serde_json default, no spaces)
   - Should match, but verify

4. **Object Key Order**:
   - JavaScript `JSON.stringify()` preserves insertion order (ES2015+)
   - Rust `serde_json` preserves struct field order
   - Should match if client creates object in same order as Rust struct

## Expected Output Format

### Client (Browser Console)
```javascript
🧠 Client header JSON (hashed payload): {"version":1,"height":12345,"prev_hash":[0,1,2,...],...}
🔍 Client header hash calculation: {
  jsonLength: 1234,
  jsonBytesLength: 1234,
  jsonPreview: "...",
  jsonBytes: [123, 34, 118, ...],
  hash: "abc123...",
  leadingZeros: 0
}
```

### Server (Droplet Logs)
```
🔍 Difficulty check (JSON): hash=abc123... leading_zeros=0, required=4
📄 Header JSON (server hashed payload, 1234 bytes): {"version":1,"height":12345,"prev_hash":[0,1,2,...],...}
📄 Header JSON bytes (first 200): [123, 34, 118, ...]
```

## Next Steps After Comparison

Once differences are identified:
1. Document the exact differences
2. Adjust client-side serialization to match server exactly
3. Or add custom serializer on server to match client
4. Re-test to verify hash matches

