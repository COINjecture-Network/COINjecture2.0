# Deployment and Testing Summary

## ✅ Completed Steps

### 1. Lambda@Edge Function Updated
- **Function**: `coinjecture-rpc-proxy`
- **Version**: 3 (published)
- **ARN**: `arn:aws:lambda:us-east-1:036160411444:function:coinjecture-rpc-proxy:3`
- **Improvements**:
  - Enhanced error handling with request ID tracking
  - Increased timeout from 5s to 10s
  - Better logging for debugging 502 errors
  - Proper OPTIONS preflight handling
  - JSON-RPC 2.0 compliant error responses

### 2. CloudFront Distribution Updated
- **Distribution ID**: `E1F9JMDDFH6L9V`
- **Status**: `InProgress` (takes 5-15 minutes to deploy)
- **Lambda Association**: Updated to version 3
- **Path Pattern**: `/api/rpc`
- **Event Type**: `viewer-request`
- **IncludeBody**: `true`

### 3. Enhanced Debug Logging
- **Server-side**: Added detailed JSON byte logging in `node/src/validator.rs`
- **Client-side**: Enhanced logging in `web/coinjecture-evolved-main/src/lib/mining.ts`
- **Debug Flag**: `localStorage.setItem('coinjecture:mining-debug', 'true')`

## 🧪 Testing Instructions

### Step 1: Wait for CloudFront Deployment
```bash
# Check deployment status
aws cloudfront get-distribution --id E1F9JMDDFH6L9V --query 'Distribution.Status' --output text

# Wait until status is "Deployed" (usually 5-15 minutes)
```

### Step 2: Enable Debug Logging
1. Open browser console (F12)
2. Run:
```javascript
localStorage.setItem('coinjecture:mining-debug', 'true');
console.log('✅ Debug logging enabled');
```

### Step 3: Test Mining
1. Navigate to: https://d1f2zzpbyxllz7.cloudfront.net/terminal
2. Run: `mine submit`
3. Watch browser console for debug output

### Step 4: Check Server Logs
```bash
ssh -i ~/.ssh/COINjecture-Key root@143.110.139.166
docker logs coinject-node 2>&1 | grep -E '🔍|📄|Difficulty check|Header JSON' | tail -50
```

### Step 5: Compare JSON Payloads

#### Client Output (Browser Console)
Look for these log entries:
- `🧠 Client header JSON (hashed payload):` - Exact JSON string
- `🔍 Client header hash calculation:` - JSON bytes, hash, leading zeros
- `🔍 Client header object (before JSON.stringify):` - Object structure

#### Server Output (Droplet Logs)
Look for these log entries:
- `🔍 Difficulty check (JSON):` - Hash and leading zeros
- `📄 Header JSON (server hashed payload, X bytes):` - Exact JSON string
- `📄 Header JSON bytes (first 200):` - Byte array representation

## 🔍 What to Compare

### 1. JSON String Comparison
Compare the exact JSON strings character-by-character:
- Client: `{"version":1,"height":12345,...}`
- Server: `{"version":1,"height":12345,...}`

### 2. Field Order
Ensure fields match Rust struct order:
```
version, height, prev_hash, timestamp, transactions_root, solutions_root,
commitment, work_score, miner, nonce, solve_time_us, verify_time_us,
time_asymmetry_ratio, solution_quality, complexity_weight, energy_estimate_joules
```

### 3. Float Precision
Check if floats are serialized differently:
- JavaScript: `1.4142135623730951` (default precision)
- Rust serde_json: `1.4142135623730951` (default precision)
- May differ in trailing zeros or scientific notation

### 4. Array Formatting
Verify byte arrays are formatted identically:
- Client: `[0,1,2,3]` (no spaces)
- Server: `[0,1,2,3]` (serde_json default, no spaces)

### 5. Byte Array Comparison
Compare the first 200 bytes from both sides:
- Client: `[123, 34, 118, ...]`
- Server: `[123, 34, 118, ...]`

## 🐛 Common Issues to Check

### Issue 1: Float Serialization
**Symptom**: Different float representations
**Example**:
- Client: `1.4142135623730951`
- Server: `1.414213562373095`

**Fix**: Use `toFixed()` or custom serializer to match precision

### Issue 2: Field Order
**Symptom**: Fields in different order
**Example**:
- Client: `{"version":1,"height":2}`
- Server: `{"height":2,"version":1}`

**Fix**: Ensure client creates object in same order as Rust struct

### Issue 3: Array Formatting
**Symptom**: Different array formatting
**Example**:
- Client: `[0,1,2]`
- Server: `[0, 1, 2]` (with spaces)

**Fix**: Use `JSON.stringify()` with no spaces (default)

### Issue 4: Object Key Order
**Symptom**: Keys in different order despite same values
**Fix**: JavaScript `JSON.stringify()` preserves insertion order (ES2015+), should match if object created in same order

## 📊 Expected Output Format

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
🔍 Client header object (before JSON.stringify): {
  version: 1,
  height: 12345,
  ...
}
```

### Server (Droplet Logs)
```
🔍 Difficulty check (JSON): hash=abc123... leading_zeros=0, required=4
📄 Header JSON (server hashed payload, 1234 bytes): {"version":1,"height":12345,"prev_hash":[0,1,2,...],...}
📄 Header JSON bytes (first 200): [123, 34, 118, ...]
```

## 🔧 Next Steps After Comparison

1. **Document Differences**: Record exact differences found
2. **Adjust Serialization**: Modify client or server to match exactly
3. **Re-test**: Verify hash matches after fix
4. **Update Code**: Commit fixes to repository

## 📝 Notes

- CloudFront deployment takes 5-15 minutes
- Lambda@Edge logs appear in CloudWatch (us-east-1 region)
- Server logs are in Docker container logs on droplets
- Debug logging can be disabled: `localStorage.removeItem('coinjecture:mining-debug')`

