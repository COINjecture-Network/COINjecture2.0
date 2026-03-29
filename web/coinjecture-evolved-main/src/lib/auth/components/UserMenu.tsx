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

type UserMenuProps = {
  /** Narrow toolbar (e.g. mobile nav): shorter label, smaller max width */
  compact?: boolean;
};

export function UserMenu({ compact }: UserMenuProps) {
  const {
    isAuthenticated,
    isLoading,
    user,
    authMethod,
    isFullyLinked,
    signOut,
    openAuthModal,
  } = useAuth();

  if (isLoading) {
    return (
      <Button
        variant="outline"
        size="sm"
        disabled
        className={
          compact
            ? 'min-w-0 px-2 border-border/60'
            : 'min-w-[5.5rem] border-border/60'
        }
      >
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
        className={
          compact
            ? 'border-border/60 bg-background/50 backdrop-blur-sm gap-1.5 px-2 shrink-0'
            : 'border-border/60 bg-background/50 backdrop-blur-sm gap-2'
        }
        onClick={() => openAuthModal('welcome')}
        aria-label="Log in"
      >
        <LogIn className="h-4 w-4 shrink-0 opacity-80" />
        <span className={compact ? 'hidden min-[380px]:inline' : undefined}>Log in</span>
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
    <DropdownMenu modal={false}>
      <DropdownMenuTrigger asChild>
        <Button
          type="button"
          variant="outline"
          size="sm"
          className={
            compact
              ? 'border-border/60 bg-background/50 font-mono text-xs gap-1.5 max-w-[7.5rem] min-w-0 px-2 shrink-0'
              : 'border-border/60 bg-background/50 font-mono text-xs gap-2 max-w-[11rem] sm:max-w-[14rem]'
          }
        >
          <span className={`h-2 w-2 shrink-0 rounded-full ${dotColor}`} />
          <span className="truncate">{compact ? truncate(displayLabel, 4, 3) : displayLabel}</span>
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" side="bottom" sideOffset={8} className="w-52">
        {user?.wallet_address && (
          <DropdownMenuItem
            onClick={() => {
              navigator.clipboard.writeText(user.wallet_address!);
            }}
          >
              Copy wallet address
            </DropdownMenuItem>
          )}
          {!user?.wallet_address && authMethod === 'email' && (
            <DropdownMenuItem
              onClick={() => {
                openAuthModal('wallet');
              }}
            >
              Link wallet
            </DropdownMenuItem>
          )}
          {!user?.email && authMethod === 'wallet' && (
            <DropdownMenuItem
              onClick={() => {
                openAuthModal('email', 'signin');
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
            }}
          >
            Sign out
          </DropdownMenuItem>
        </DropdownMenuContent>
    </DropdownMenu>
  );
}
