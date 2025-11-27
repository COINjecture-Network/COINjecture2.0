// Lambda@Edge function to proxy RPC requests to any node
// This runs at CloudFront edge locations and can proxy to any HTTP endpoint

const http = require('http');

exports.handler = async (event) => {
    const request = event.Records[0].cf.request;
    const uri = request.uri;
    
    // Only handle /api/rpc requests
    if (!uri.startsWith('/api/rpc')) {
        return request;
    }
    
    // Parse target RPC endpoint from query string
    let targetHost = '143.110.139.166';
    let targetPort = 9933;
    
    // Extract target from query string: /api/rpc?target=host:port
    if (request.querystring) {
        const params = new URLSearchParams(request.querystring);
        const target = params.get('target');
        if (target) {
            const parts = target.split(':');
            targetHost = parts[0];
            targetPort = parts[1] ? parseInt(parts[1]) : 9933;
        }
    }
    
    // Get request body
    const body = request.body && request.body.data 
        ? Buffer.from(request.body.data, request.body.encoding === 'base64' ? 'base64' : 'utf8').toString()
        : '';
    
    // Make HTTP request to target RPC endpoint
    return new Promise((resolve, reject) => {
        const options = {
            hostname: targetHost,
            port: targetPort,
            path: '/',
            method: request.method,
            headers: {
                'Content-Type': 'application/json',
                'Content-Length': Buffer.byteLength(body)
            },
            timeout: 5000
        };
        
        const req = http.request(options, (res) => {
            let responseBody = '';
            
            res.on('data', (chunk) => {
                responseBody += chunk;
            });
            
            res.on('end', () => {
                resolve({
                    status: res.statusCode.toString(),
                    statusDescription: res.statusMessage || 'OK',
                    headers: {
                        'content-type': [{
                            key: 'Content-Type',
                            value: 'application/json'
                        }],
                        'access-control-allow-origin': [{
                            key: 'Access-Control-Allow-Origin',
                            value: '*'
                        }],
                        'access-control-allow-methods': [{
                            key: 'Access-Control-Allow-Methods',
                            value: 'POST, GET, OPTIONS'
                        }],
                        'access-control-allow-headers': [{
                            key: 'Access-Control-Allow-Headers',
                            value: 'Content-Type'
                        }]
                    },
                    body: responseBody
                });
            });
        });
        
        req.on('error', (error) => {
            resolve({
                status: '502',
                statusDescription: 'Bad Gateway',
                headers: {
                    'content-type': [{
                        key: 'Content-Type',
                        value: 'application/json'
                    }],
                    'access-control-allow-origin': [{
                        key: 'Access-Control-Allow-Origin',
                        value: '*'
                    }]
                },
                body: JSON.stringify({ 
                    error: { 
                        code: -32000, 
                        message: `Failed to connect to RPC endpoint: ${error.message}` 
                    } 
                })
            });
        });
        
        req.setTimeout(5000, () => {
            req.destroy();
            resolve({
                status: '504',
                statusDescription: 'Gateway Timeout',
                headers: {
                    'content-type': [{ key: 'Content-Type', value: 'application/json' }],
                    'access-control-allow-origin': [{ key: 'Access-Control-Allow-Origin', value: '*' }]
                },
                body: JSON.stringify({ error: { code: -32000, message: 'Request timeout' } })
            });
        });
        
        if (body) {
            req.write(body);
        }
        req.end();
    });
};

