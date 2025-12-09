// Lambda@Edge function to proxy RPC requests to any node
// This runs at CloudFront edge locations and can proxy to any HTTP endpoint

const http = require('http');

exports.handler = async (event) => {
    try {
        const request = event.Records[0].cf.request;
        const uri = request.uri;
        
    // Only handle /api/rpc requests
    if (!uri.startsWith('/api/rpc')) {
        return request;
    }
    
    // Ensure POST method for RPC requests (JSON-RPC requires POST)
    if (request.method !== 'POST' && request.method !== 'OPTIONS') {
        return {
            status: '405',
            statusDescription: 'Method Not Allowed',
            headers: {
                'content-type': [{ key: 'Content-Type', value: 'application/json' }],
                'access-control-allow-origin': [{ key: 'Access-Control-Allow-Origin', value: '*' }],
                'allow': [{ key: 'Allow', value: 'POST, OPTIONS' }]
            },
            body: JSON.stringify({
                jsonrpc: '2.0',
                error: {
                    code: -32600,
                    message: 'Used HTTP Method is not allowed. POST is required',
                    data: { method: request.method || 'UNKNOWN' }
                },
                id: null
            })
        };
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
    
    // Get request body (handles both base64 and text encoding)
    let body = '';
    if (request.body) {
        // Check if body was truncated (Lambda@Edge viewer-request max is 1MB)
        if (request.body.inputTruncated === true) {
            console.error(`[${requestId}] Request body truncated (too large for Lambda@Edge)`);
            return {
                status: '413',
                statusDescription: 'Payload Too Large',
                headers: {
                    'content-type': [{ key: 'Content-Type', value: 'application/json' }],
                    'access-control-allow-origin': [{ key: 'Access-Control-Allow-Origin', value: '*' }]
                },
                body: JSON.stringify({
                    jsonrpc: '2.0',
                    error: {
                        code: -32600,
                        message: 'Request body too large for Lambda@Edge (max 1MB)',
                        data: { requestId }
                    },
                    id: null
                })
            };
        }
        
        if (request.body.data) {
            body = Buffer.from(
                request.body.data,
                request.body.encoding === 'base64' ? 'base64' : 'utf8'
            ).toString('utf8');
        } else if (request.body.inputTruncated === false) {
            // Body might be in request.body directly
            body = typeof request.body === 'string' ? request.body : JSON.stringify(request.body);
        }
    }
    
    // Log request details (for debugging - will appear in CloudWatch)
    const requestId = event.Records[0].cf.config.requestId || 'unknown';
    console.log(`[${requestId}] Proxying ${request.method} to ${targetHost}:${targetPort}`);
    
    // Make HTTP request to target RPC endpoint
    return new Promise((resolve) => {
        const options = {
            hostname: targetHost,
            port: targetPort,
            path: '/',
            method: request.method || 'POST',
            headers: {
                'Content-Type': 'application/json',
                'Content-Length': Buffer.byteLength(body, 'utf8'),
                'User-Agent': 'COINjecture-CloudFront-Proxy/1.0'
            },
            timeout: 5000 // Lambda@Edge viewer-request timeout is 5 seconds max
        };
        
        const startTime = Date.now();
        const req = http.request(options, (res) => {
            let responseBody = '';
            const statusCode = res.statusCode || 500;
            
            // Log response status
            console.log(`[${requestId}] Response: ${statusCode} from ${targetHost}:${targetPort} (${Date.now() - startTime}ms)`);
            
            res.on('data', (chunk) => {
                responseBody += chunk.toString('utf8');
            });
            
            res.on('end', () => {
                // Log response size
                console.log(`[${requestId}] Response body size: ${responseBody.length} bytes`);
                
                resolve({
                    status: statusCode.toString(),
                    statusDescription: res.statusMessage || 'OK',
                    headers: {
                        'content-type': [{
                            key: 'Content-Type',
                            value: res.headers['content-type'] || 'application/json'
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
            
            res.on('error', (error) => {
                console.error(`[${requestId}] Response stream error:`, error.message);
                resolve({
                    status: '502',
                    statusDescription: 'Bad Gateway',
                    headers: {
                        'content-type': [{ key: 'Content-Type', value: 'application/json' }],
                        'access-control-allow-origin': [{ key: 'Access-Control-Allow-Origin', value: '*' }]
                    },
                    body: JSON.stringify({
                        jsonrpc: '2.0',
                        error: {
                            code: -32000,
                            message: `Response stream error: ${error.message}`,
                            data: { target: `${targetHost}:${targetPort}`, requestId }
                        },
                        id: null
                    })
                });
            });
        });
        
        req.on('error', (error) => {
            const errorMessage = error.message || 'Unknown error';
            const errorCode = error.code || 'UNKNOWN';
            console.error(`[${requestId}] Request error:`, {
                message: errorMessage,
                code: errorCode,
                target: `${targetHost}:${targetPort}`,
                duration: Date.now() - startTime
            });
            
            resolve({
                status: '502',
                statusDescription: 'Bad Gateway',
                headers: {
                    'content-type': [{ key: 'Content-Type', value: 'application/json' }],
                    'access-control-allow-origin': [{ key: 'Access-Control-Allow-Origin', value: '*' }]
                },
                body: JSON.stringify({
                    jsonrpc: '2.0',
                    error: {
                        code: -32000,
                        message: `Failed to connect to RPC endpoint: ${errorMessage}`,
                        data: {
                            target: `${targetHost}:${targetPort}`,
                            errorCode: errorCode,
                            requestId
                        }
                    },
                    id: null
                })
            });
        });
        
        // Lambda@Edge viewer-request timeout is 5 seconds max
        req.setTimeout(4500, () => {
            console.error(`[${requestId}] Request timeout after 4.5s to ${targetHost}:${targetPort}`);
            req.destroy();
            resolve({
                status: '504',
                statusDescription: 'Gateway Timeout',
                headers: {
                    'content-type': [{ key: 'Content-Type', value: 'application/json' }],
                    'access-control-allow-origin': [{ key: 'Access-Control-Allow-Origin', value: '*' }]
                },
                body: JSON.stringify({
                    jsonrpc: '2.0',
                    error: {
                        code: -32000,
                        message: 'Request timeout (Lambda@Edge viewer-request limit: 5s)',
                        data: {
                            target: `${targetHost}:${targetPort}`,
                            requestId
                        }
                    },
                    id: null
                })
            });
        });
        
        // Write request body
        if (body) {
            try {
                req.write(body, 'utf8');
            } catch (writeError) {
                console.error(`[${requestId}] Write error:`, writeError.message);
                req.destroy();
                resolve({
                    status: '500',
                    statusDescription: 'Internal Server Error',
                    headers: {
                        'content-type': [{ key: 'Content-Type', value: 'application/json' }],
                        'access-control-allow-origin': [{ key: 'Access-Control-Allow-Origin', value: '*' }]
                    },
                    body: JSON.stringify({
                        jsonrpc: '2.0',
                        error: {
                            code: -32603,
                            message: `Failed to write request body: ${writeError.message}`,
                            data: { requestId }
                        },
                        id: null
                    })
                });
                return;
            }
        }
        
        req.end();
    }).catch((error) => {
        // Catch any unhandled promise rejections
        console.error('Unhandled error in Lambda:', error);
        return {
            status: '500',
            statusDescription: 'Internal Server Error',
            headers: {
                'content-type': [{ key: 'Content-Type', value: 'application/json' }],
                'access-control-allow-origin': [{ key: 'Access-Control-Allow-Origin', value: '*' }]
            },
            body: JSON.stringify({
                jsonrpc: '2.0',
                error: {
                    code: -32603,
                    message: `Lambda execution error: ${error.message || 'Unknown error'}`,
                    data: { errorType: error.name || 'Error' }
                },
                id: null
            })
        };
    });
    } catch (error) {
        // Catch any synchronous errors
        console.error('Synchronous error in Lambda:', error);
        return {
            status: '500',
            statusDescription: 'Internal Server Error',
            headers: {
                'content-type': [{ key: 'Content-Type', value: 'application/json' }],
                'access-control-allow-origin': [{ key: 'Access-Control-Allow-Origin', value: '*' }]
            },
            body: JSON.stringify({
                jsonrpc: '2.0',
                error: {
                    code: -32603,
                    message: `Lambda execution error: ${error.message || 'Unknown error'}`,
                    data: { errorType: error.name || 'Error' }
                },
                id: null
            })
        };
    }
};

