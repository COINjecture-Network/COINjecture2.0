import { Github, Twitter, MessageCircle } from "lucide-react";

export const Footer = () => {
  return (
    <footer className="border-t border-border/50 py-12">
      <div className="container mx-auto px-6">
        <div className="grid grid-cols-1 md:grid-cols-4 gap-8 mb-8">
          <div>
            <div className="text-xl font-bold gradient-text mb-4">COINjecture</div>
            <p className="text-sm text-muted-foreground">
              Utility-based computational work blockchain powered by $BEANS
            </p>
          </div>
          
          <div>
            <h4 className="font-semibold mb-4">Product</h4>
            <ul className="space-y-2 text-sm text-muted-foreground">
              <li><a href="#terminal" className="hover:text-foreground transition-colors">Terminal</a></li>
              <li><a href="#api" className="hover:text-foreground transition-colors">API Docs</a></li>
              <li><a href="#metrics" className="hover:text-foreground transition-colors">Metrics</a></li>
              <li><a href="#marketplace" className="hover:text-foreground transition-colors">Marketplace</a></li>
            </ul>
          </div>
          
          <div>
            <h4 className="font-semibold mb-4">Resources</h4>
            <ul className="space-y-2 text-sm text-muted-foreground">
              <li><a href="#api" className="hover:text-foreground transition-colors">Documentation</a></li>
              <li><a href="/whitepaper" className="hover:text-foreground transition-colors">Whitepaper</a></li>
              <li><a href="https://github.com/Quigles1337/COINjecture2.0" target="_blank" rel="noopener noreferrer" className="hover:text-foreground transition-colors">GitHub</a></li>
              <li><a href="#" className="hover:text-foreground transition-colors">Support</a></li>
            </ul>
          </div>
          
          <div>
            <h4 className="font-semibold mb-4">Community</h4>
            <div className="flex gap-4">
              <a href="https://github.com/Quigles1337/COINjecture2.0" target="_blank" rel="noopener noreferrer" className="p-2 rounded-lg hover:bg-muted transition-colors" aria-label="GitHub">
                <Github className="h-5 w-5" />
              </a>
              <a href="https://x.com/COINjecture" target="_blank" rel="noopener noreferrer" className="p-2 rounded-lg hover:bg-muted transition-colors" aria-label="Twitter">
                <Twitter className="h-5 w-5" />
              </a>
              <a href="#" className="p-2 rounded-lg hover:bg-muted transition-colors" aria-label="Discord">
                <MessageCircle className="h-5 w-5" />
              </a>
            </div>
          </div>
        </div>
        
        <div className="pt-8 border-t border-border/50 flex flex-col md:flex-row justify-between items-center gap-4 text-sm text-muted-foreground">
          <p>© 2025 COINjecture. All rights reserved.</p>
          <div className="flex gap-6">
            <a href="#" className="hover:text-foreground transition-colors">Privacy Policy</a>
            <a href="#" className="hover:text-foreground transition-colors">Terms of Service</a>
          </div>
        </div>
      </div>
    </footer>
  );
};
