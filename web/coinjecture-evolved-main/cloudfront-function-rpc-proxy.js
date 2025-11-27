// CloudFront Function to proxy RPC requests to any node
// This function rewrites requests to /api/rpc to proxy to the actual RPC endpoint
function handler(event) {
    var request = event.request;
    var uri = request.uri;
    
    // Only handle /api/rpc requests
    if (uri.startsWith('/api/rpc')) {
        // Get target RPC endpoint from query string or header
        // Default to first node if not specified
        var targetHost = '143.110.139.166:9933';
        
        // Check for target in query string
        if (request.querystring && request.querystring.target) {
            targetHost = decodeURIComponent(request.querystring.target);
        }
        
        // Check for target in custom header
        if (request.headers['x-rpc-target']) {
            targetHost = request.headers['x-rpc-target'].value;
        }
        
        // Rewrite to proxy through CloudFront origin
        // We'll use a custom origin that proxies to the RPC endpoint
        request.uri = '/rpc-proxy';
        request.headers['host'] = {value: targetHost};
        request.headers['x-forwarded-to'] = {value: 'http://' + targetHost};
    }
    
    return request;
}
