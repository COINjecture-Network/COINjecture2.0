// Lambda@Edge function to proxy RPC requests to any node
exports.handler = async (event) => {
    const request = event.Records[0].cf.request;
    
    // Only handle /api/rpc requests
    if (!request.uri.startsWith('/api/rpc')) {
        return request;
    }
    
    // Handle OPTIONS preflight requests
    if (request.method === 'OPTIONS') {
        return {
            status: '200',
            statusDescription: 'OK',
            headers: {
                'access-control-allow-origin': [{ key: 'Access-Control-Allow-Origin', value: '*' }],
                'access-control-allow-methods': [{ key: 'Access-Control-Allow-Methods', value: 'POST, GET, OPTIONS' }],
                'access-control-allow-headers': [{ key: 'Access-Control-Allow-Headers', value: 'Content-Type' }],
                'access-control-max-age': [{ key: 'Access-Control-Max-Age', value: '86400' }]
            },
            body: ''
        };
    }
    
    // Parse target from query string
    let targetHost = '143.110.139.166';
    let targetPort = 9933;
    
    if (request.querystring) {
        const params = new URLSearchParams(request.querystring);
        const target = params.get('target');
        if (target) {
            const parts = target.split(':');
            targetHost = parts[0];
            targetPort = parts[1] ? parseInt(parts[1]) : 9933;
        }
    }
    
    // Get request body (handles both base64 and text encoding)
    let body = '';
    if (request.body) {
        if (request.body.data) {
            if (request.body.encoding === 'base64') {
                body = Buffer.from(request.body.data, 'base64').toString('utf8');
            } else {
                body = request.body.data;
            }
        } else if (request.body.inputTruncated) {
            // Body was truncated (too large for Lambda@Edge)
            return {
                status: '413',
                statusDescription: 'Payload Too Large',
                headers: {
                    'content-type': [{ key: 'Content-Type', value: 'application/json' }],
                    'access-control-allow-origin': [{ key: 'Access-Control-Allow-Origin', value: '*' }]
                },
                body: JSON.stringify({ error: 'Request body too large for Lambda@Edge (max 1MB)' })
            };
        }
    }
    
    // Use Node.js http module to proxy request
    const http = require('http');
    
    return new Promise((resolve, reject) => {
        const options = {
            hostname: targetHost,
            port: targetPort,
            path: '/',
            method: request.method,
            headers: {
                'Content-Type': 'application/json'
            }
        };
        
        // Only set Content-Length if body exists
        if (body) {
            options.headers['Content-Length'] = Buffer.byteLength(body);
        }
        
        const req = http.request(options, (res) => {
            let responseBody = '';
            res.on('data', (chunk) => { responseBody += chunk; });
            res.on('end', () => {
                resolve({
                    status: res.statusCode.toString(),
                    statusDescription: res.statusMessage,
                    headers: {
                        'content-type': [{ key: 'Content-Type', value: 'application/json' }],
                        'access-control-allow-origin': [{ key: 'Access-Control-Allow-Origin', value: '*' }],
                        'access-control-allow-methods': [{ key: 'Access-Control-Allow-Methods', value: 'POST, GET, OPTIONS' }],
                        'access-control-allow-headers': [{ key: 'Access-Control-Allow-Headers', value: 'Content-Type' }]
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
                    'content-type': [{ key: 'Content-Type', value: 'application/json' }],
                    'access-control-allow-origin': [{ key: 'Access-Control-Allow-Origin', value: '*' }]
                },
                body: JSON.stringify({ error: error.message, code: -32603 })
            });
        });
        
        // Set timeout
        req.setTimeout(5000, () => {
            req.destroy();
            resolve({
                status: '504',
                statusDescription: 'Gateway Timeout',
                headers: {
                    'content-type': [{ key: 'Content-Type', value: 'application/json' }],
                    'access-control-allow-origin': [{ key: 'Access-Control-Allow-Origin', value: '*' }]
                },
                body: JSON.stringify({ error: 'Request timeout', code: -32603 })
            });
        });
        
        if (body) req.write(body);
        req.end();
    });
};
