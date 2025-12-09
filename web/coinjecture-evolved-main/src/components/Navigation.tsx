import { NavLink } from "@/components/NavLink";
import { useNavigate } from "react-router-dom";
import { Button } from "@/components/ui/button";
import { ThemeToggle } from "@/components/ThemeToggle";
import { Menu, X, Wallet } from "lucide-react";
import { useState } from "react";
import { useWallet } from "@/contexts/WalletContext";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";

export const Navigation = () => {
  const [isOpen, setIsOpen] = useState(false);
  const navigate = useNavigate();
  const { accounts, selectedAccount, setSelectedAccount } = useWallet();
  const selectedKeyPair = selectedAccount ? accounts[selectedAccount] : null;

  return (
    <nav className="fixed top-0 left-0 right-0 z-50 glass-effect">
      <div className="container mx-auto px-6 py-4">
        <div className="flex items-center justify-between">
          <NavLink to="/" className="flex items-center gap-2">
            <div className="text-2xl font-bold text-primary">COINjecture</div>
            <span className="text-xs text-primary border border-primary px-2 py-0.5 rounded-full">$BEANS</span>
          </NavLink>

          {/* Desktop Navigation */}
          <div className="hidden md:flex items-center gap-6">
            <NavLink 
              to="/terminal" 
              className="text-sm text-muted-foreground hover:text-foreground transition-colors"
            >
              Terminal
            </NavLink>
            <NavLink 
              to="/api" 
              className="text-sm text-muted-foreground hover:text-foreground transition-colors"
            >
              API Docs
            </NavLink>
            <NavLink 
              to="/metrics" 
              className="text-sm text-muted-foreground hover:text-foreground transition-colors"
            >
              Metrics
            </NavLink>
            <NavLink 
              to="/marketplace" 
              className="text-sm text-muted-foreground hover:text-foreground transition-colors"
            >
              Marketplace
            </NavLink>
            <NavLink 
              to="/whitepaper" 
              className="text-sm text-muted-foreground hover:text-foreground transition-colors"
            >
              Whitepaper
            </NavLink>
            <ThemeToggle />
            {selectedKeyPair ? (
              <DropdownMenu>
                <DropdownMenuTrigger asChild>
                  <Button variant="default" size="sm" className="glow-hover">
                    <Wallet className="h-4 w-4 mr-2" />
                    {selectedAccount}
                  </Button>
                </DropdownMenuTrigger>
                <DropdownMenuContent align="end">
                  <DropdownMenuLabel>
                    <div className="text-xs font-mono text-muted-foreground">
                      {selectedKeyPair.address.slice(0, 16)}...{selectedKeyPair.address.slice(-8)}
                    </div>
                  </DropdownMenuLabel>
                  <DropdownMenuSeparator />
                  <DropdownMenuItem onClick={() => navigate("/wallet")}>
                    <Wallet className="h-4 w-4 mr-2" />
                    Manage Wallet
                  </DropdownMenuItem>
                  <DropdownMenuItem onClick={() => setSelectedAccount(null)}>
                    Disconnect
                  </DropdownMenuItem>
                </DropdownMenuContent>
              </DropdownMenu>
            ) : (
              <Button variant="default" size="sm" className="glow-hover" onClick={() => navigate("/wallet")}>
                <Wallet className="h-4 w-4 mr-2" />
                Connect Wallet
              </Button>
            )}
          </div>

          {/* Mobile Menu Button */}
          <button
            onClick={() => setIsOpen(!isOpen)}
            className="md:hidden text-foreground"
          >
            {isOpen ? <X size={24} /> : <Menu size={24} />}
          </button>
        </div>

        {/* Mobile Navigation */}
        {isOpen && (
          <div className="md:hidden pt-4 pb-3 space-y-3 animate-fade-in">
            <NavLink 
              to="/terminal" 
              className="block text-sm text-muted-foreground hover:text-foreground transition-colors"
              onClick={() => setIsOpen(false)}
            >
              Terminal
            </NavLink>
            <NavLink 
              to="/api" 
              className="block text-sm text-muted-foreground hover:text-foreground transition-colors"
              onClick={() => setIsOpen(false)}
            >
              API Docs
            </NavLink>
            <NavLink 
              to="/metrics" 
              className="block text-sm text-muted-foreground hover:text-foreground transition-colors"
              onClick={() => setIsOpen(false)}
            >
              Metrics
            </NavLink>
            <NavLink 
              to="/marketplace" 
              className="block text-sm text-muted-foreground hover:text-foreground transition-colors"
              onClick={() => setIsOpen(false)}
            >
              Marketplace
            </NavLink>
            <NavLink 
              to="/whitepaper" 
              className="block text-sm text-muted-foreground hover:text-foreground transition-colors"
              onClick={() => setIsOpen(false)}
            >
              Whitepaper
            </NavLink>
            <div className="flex items-center gap-2 pt-2">
              <ThemeToggle />
              <Button 
                variant="default" 
                size="sm" 
                className="flex-1"
                onClick={() => {
                  navigate("/wallet");
                  setIsOpen(false);
                }}
              >
                <Wallet className="h-4 w-4 mr-2" />
                {selectedKeyPair ? selectedAccount : "Connect Wallet"}
              </Button>
            </div>
          </div>
        )}
      </div>
    </nav>
  );
};
