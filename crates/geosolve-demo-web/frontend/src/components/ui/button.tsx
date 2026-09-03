// SPDX-License-Identifier: GPL-3.0-or-later
import { forwardRef, type ButtonHTMLAttributes } from "react";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "../../lib/cn";

const variants = cva("inline-flex items-center justify-center gap-2 rounded-md text-sm font-medium outline-none transition-colors focus-visible:ring-2 focus-visible:ring-accent disabled:pointer-events-none disabled:opacity-45", {
  variants: {
    variant: {
      default: "bg-accent text-neutral-950 hover:bg-amber-300",
      secondary: "border border-border bg-raised text-foreground hover:bg-neutral-700",
      ghost: "text-muted hover:bg-raised hover:text-foreground",
      danger: "bg-danger text-white hover:brightness-110",
    },
    size: { default: "h-8 px-3", icon: "size-9 p-0", compact: "h-7 px-2 text-xs" },
  },
  defaultVariants: { variant: "secondary", size: "default" },
});

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement>, VariantProps<typeof variants> {}

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(({ className, variant, size, ...props }, ref) => (
  <button ref={ref} className={cn(variants({ variant, size }), className)} {...props} />
));
Button.displayName = "Button";
