import { NavLink } from "@/components/NavLink";
import { useLocation, useNavigate } from "react-router-dom";
import { Button } from "@/components/ui/button";
import { ThemeToggle } from "@/components/ThemeToggle";
import { Menu, Wallet, Pickaxe, BadgeDollarSign, ChevronDown } from "lucide-react";
import { useState } from "react";
import { useWallet } from "@/contexts/WalletContext";
import { AuthSettingsButton, UserMenu } from "@/lib/auth";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Sheet, SheetContent, SheetHeader, SheetTitle } from "@/components/ui/sheet";

export const Navigation = () => {
  const [mobileMenuOpen, setMobileMenuOpen] = useState(false);
  const navigate = useNavigate();
  const location = useLocation();
  const { accounts, selectedAccount, setSelectedAccount } = useWallet();
  const selectedKeyPair = selectedAccount ? accounts[selectedAccount] : null;
  const desktopNavItems = [
    { to: "/solver-lab", label: "Solver Lab" },
    { to: "/api", label: "API Docs" },
    { to: "/metrics", label: "Metrics" },
    { to: "/marketplace", label: "Marketplace" },
    { to: "/whitepaper", label: "Whitepaper" },
  ];
  const activeDesktopNavItem = desktopNavItems.find(({ to }) => location.pathname === to);

  return (
    <nav className="fixed top-0 left-0 right-0 z-50 glass-navbar border-b border-border/40">
      <div className="container mx-auto px-4 sm:px-6 py-3 sm:py-4">
        <div className="flex items-center justify-between gap-2 min-w-0">
          <NavLink to="/" className="flex min-w-0 shrink items-center gap-1.5 sm:gap-2">
            <div className="text-lg sm:text-xl md:text-2xl font-brand font-extrabold text-primary tracking-tight truncate">
              COINjecture
            </div>
            <span className="hidden sm:inline-flex shrink-0 text-xs text-primary border border-primary px-2 py-0.5 rounded-full">
              $BEANS
            </span>
          </NavLink>

          {/* Desktop Navigation */}
          <div className="hidden md:flex items-center gap-6">
            <div className="role-segment min-w-[240px]">
              <NavLink
                to="/solver-lab"
                className="role-segment-item flex items-center justify-center gap-2"
                activeClassName="bg-background text-foreground shadow-sm"
              >
                <Pickaxe className="h-4 w-4" />
                Earn
              </NavLink>
              <NavLink
                to="/bounty-submit"
                className="role-segment-item flex items-center justify-center gap-2"
                activeClassName="bg-background text-foreground shadow-sm"
              >
                <BadgeDollarSign className="h-4 w-4" />
                Post
              </NavLink>
            </div>
            <DropdownMenu modal={false}>
              <DropdownMenuTrigger asChild>
                <Button
                  variant="ghost"
                  className="h-10 rounded-full border border-border/60 bg-background/70 px-4 text-sm font-medium text-muted-foreground hover:bg-muted/70 hover:text-foreground"
                >
                  {activeDesktopNavItem?.label ?? "Explore"}
                  <ChevronDown className="ml-2 h-4 w-4" />
                </Button>
              </DropdownMenuTrigger>
              <DropdownMenuContent align="start" side="bottom" sideOffset={8} className="w-56">
                <DropdownMenuLabel>Explore COINjecture</DropdownMenuLabel>
                <DropdownMenuSeparator />
                {desktopNavItems.map((item) => (
                  <DropdownMenuItem key={item.to} onClick={() => navigate(item.to)}>
                    {item.label}
                  </DropdownMenuItem>
                ))}
              </DropdownMenuContent>
            </DropdownMenu>
            <div className="flex items-center gap-3 shrink-0">
              <ThemeToggle />
              <AuthSettingsButton />
              <UserMenu />
            </div>
            {selectedKeyPair ? (
              <DropdownMenu modal={false}>
                <DropdownMenuTrigger asChild>
                  <Button variant="default" size="sm" className="glow-hover gentle-animation">
                    <Wallet className="h-4 w-4 mr-2" />
                    {selectedAccount}
                  </Button>
                </DropdownMenuTrigger>
                <DropdownMenuContent align="end" side="bottom" sideOffset={8}>
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
              <Button variant="default" size="sm" className="glow-hover gentle-animation" onClick={() => navigate("/wallet")}>
                <Wallet className="h-4 w-4 mr-2" />
                Connect Wallet
              </Button>
            )}
          </div>

          {/* Mobile: compact account + slide-out menu (sheet sits above the bar, z-[60]) */}
          <div className="flex md:hidden items-center gap-1.5 shrink-0">
            <AuthSettingsButton compact />
            <UserMenu compact />
            <button
              type="button"
              onClick={() => setMobileMenuOpen(true)}
              className="text-foreground p-2 -mr-1 rounded-md hover:bg-muted/60 touch-manipulation"
              aria-expanded={mobileMenuOpen}
              aria-controls="site-mobile-nav"
              aria-label="Open menu"
            >
              <Menu size={22} />
            </button>
          </div>
        </div>
      </div>

      <Sheet open={mobileMenuOpen} onOpenChange={setMobileMenuOpen}>
        <SheetContent
          id="site-mobile-nav"
          side="right"
          className="flex w-[min(100vw-1rem,20rem)] flex-col sm:max-w-sm"
        >
          <SheetHeader className="text-left">
            <SheetTitle className="font-brand">Menu</SheetTitle>
          </SheetHeader>
          <nav className="mt-6 flex flex-col gap-1" aria-label="Mobile">
            <div className="role-segment mb-4">
              <NavLink
                to="/solver-lab"
                className="role-segment-item flex items-center justify-center gap-2"
                activeClassName="bg-background text-foreground shadow-sm"
                onClick={() => setMobileMenuOpen(false)}
              >
                <Pickaxe className="h-4 w-4" />
                Earn
              </NavLink>
              <NavLink
                to="/bounty-submit"
                className="role-segment-item flex items-center justify-center gap-2"
                activeClassName="bg-background text-foreground shadow-sm"
                onClick={() => setMobileMenuOpen(false)}
              >
                <BadgeDollarSign className="h-4 w-4" />
                Post
              </NavLink>
            </div>
            <NavLink
              to="/solver-lab"
              className="rounded-md px-3 py-2.5 text-sm text-muted-foreground hover:bg-muted hover:text-foreground"
              onClick={() => setMobileMenuOpen(false)}
            >
              Solver Lab
            </NavLink>
            <NavLink
              to="/api"
              className="rounded-md px-3 py-2.5 text-sm text-muted-foreground hover:bg-muted hover:text-foreground"
              onClick={() => setMobileMenuOpen(false)}
            >
              API Docs
            </NavLink>
            <NavLink
              to="/metrics"
              className="rounded-md px-3 py-2.5 text-sm text-muted-foreground hover:bg-muted hover:text-foreground"
              onClick={() => setMobileMenuOpen(false)}
            >
              Metrics
            </NavLink>
            <NavLink
              to="/marketplace"
              className="rounded-md px-3 py-2.5 text-sm text-muted-foreground hover:bg-muted hover:text-foreground"
              onClick={() => setMobileMenuOpen(false)}
            >
              Marketplace
            </NavLink>
            <NavLink
              to="/whitepaper"
              className="rounded-md px-3 py-2.5 text-sm text-muted-foreground hover:bg-muted hover:text-foreground"
              onClick={() => setMobileMenuOpen(false)}
            >
              Whitepaper
            </NavLink>
          </nav>
          <div className="mt-4 flex shrink-0 flex-col gap-3 border-t border-border/60 pt-6">
            <div className="flex items-center justify-between gap-2 px-1">
              <span className="text-sm text-muted-foreground">Theme</span>
              <ThemeToggle />
            </div>
            {selectedKeyPair ? (
              <DropdownMenu modal={false}>
                <DropdownMenuTrigger asChild>
                  <Button variant="default" size="sm" className="w-full justify-start gentle-animation">
                    <Wallet className="h-4 w-4 mr-2 shrink-0" />
                    <span className="truncate">{selectedAccount}</span>
                  </Button>
                </DropdownMenuTrigger>
                <DropdownMenuContent align="start" side="top" sideOffset={8} className="min-w-[12rem]">
                  <DropdownMenuLabel>
                    <div className="text-xs font-mono text-muted-foreground break-all">
                      {selectedKeyPair.address.slice(0, 16)}…{selectedKeyPair.address.slice(-8)}
                    </div>
                  </DropdownMenuLabel>
                  <DropdownMenuSeparator />
                  <DropdownMenuItem
                    onClick={() => {
                      navigate("/wallet");
                      setMobileMenuOpen(false);
                    }}
                  >
                    <Wallet className="h-4 w-4 mr-2" />
                    Manage Wallet
                  </DropdownMenuItem>
                  <DropdownMenuItem
                    onClick={() => {
                      setSelectedAccount(null);
                      setMobileMenuOpen(false);
                    }}
                  >
                    Disconnect
                  </DropdownMenuItem>
                </DropdownMenuContent>
              </DropdownMenu>
            ) : (
              <Button
                variant="default"
                size="sm"
                className="w-full gentle-animation"
                onClick={() => {
                  navigate("/wallet");
                  setMobileMenuOpen(false);
                }}
              >
                <Wallet className="h-4 w-4 mr-2" />
                Connect Wallet
              </Button>
            )}
          </div>
        </SheetContent>
      </Sheet>
    </nav>
  );
};
