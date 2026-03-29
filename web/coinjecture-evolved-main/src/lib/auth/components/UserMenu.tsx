import { useState, useRef, useEffect } from 'react';
import { LogIn } from 'lucide-react';
import { useAuth } from '../useAuth';
import { Button } from '@/components/ui/button';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';

function truncate(s: string, head = 6, tail = 4): string {
  if (s.length <= head + tail + 3) return s;
  return `${s.slice(0, head)}...${s.slice(-tail)}`;
}

export function UserMenu() {
  const {
    isAuthenticated,
    isLoading,
    user,
    authMethod,
    isFullyLinked,
    signOut,
    openAuthModal,
  } = useAuth();

  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener('mousedown', handler);
    return () => document.removeEventListener('mousedown', handler);
  }, []);

  if (isLoading) {
    return (
      <Button variant="outline" size="sm" disabled className="min-w-[5.5rem] border-border/60">
        …
      </Button>
    );
  }

  if (!isAuthenticated) {
    return (
      <Button
        type="button"
        variant="outline"
        size="sm"
        className="border-border/60 bg-background/50 backdrop-blur-sm gap-2"
        onClick={() => openAuthModal('welcome')}
      >
        <LogIn className="h-4 w-4 opacity-80" />
        Log in
      </Button>
    );
  }

  const dotColor = isFullyLinked
    ? 'bg-blue-400'
    : user?.wallet_address
      ? 'bg-emerald-400'
      : 'bg-amber-400';

  const displayLabel = user?.wallet_address
    ? truncate(user.wallet_address)
    : user?.email
      ? truncate(user.email, 10, 0)
      : 'Account';

  return (
    <div ref={ref}>
      <DropdownMenu open={open} onOpenChange={setOpen}>
        <DropdownMenuTrigger asChild>
          <Button
            type="button"
            variant="outline"
            size="sm"
            className="border-border/60 bg-background/50 font-mono text-xs gap-2 max-w-[11rem] sm:max-w-[14rem]"
          >
            <span className={`h-2 w-2 shrink-0 rounded-full ${dotColor}`} />
            <span className="truncate">{displayLabel}</span>
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end" className="w-52">
          {user?.wallet_address && (
            <DropdownMenuItem
              onClick={() => {
                navigator.clipboard.writeText(user.wallet_address!);
                setOpen(false);
              }}
            >
              Copy wallet address
            </DropdownMenuItem>
          )}
          {!user?.wallet_address && authMethod === 'email' && (
            <DropdownMenuItem
              onClick={() => {
                openAuthModal('wallet');
                setOpen(false);
              }}
            >
              Link wallet
            </DropdownMenuItem>
          )}
          {!user?.email && authMethod === 'wallet' && (
            <DropdownMenuItem
              onClick={() => {
                openAuthModal('email', 'signin');
                setOpen(false);
              }}
            >
              Add email
            </DropdownMenuItem>
          )}
          <DropdownMenuSeparator />
          <DropdownMenuItem
            className="text-destructive focus:text-destructive"
            onClick={() => {
              signOut();
              setOpen(false);
            }}
          >
            Sign out
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
    </div>
  );
}
