// SPDX-License-Identifier: GPL-3.0-or-later
import * as TabsPrimitive from "@radix-ui/react-tabs";
import { cn } from "../../lib/cn";

export const Tabs = TabsPrimitive.Root;
export const TabsList = ({ className, ...props }: TabsPrimitive.TabsListProps) => <TabsPrimitive.List className={cn("flex border-b border-border bg-surface px-1", className)} {...props} />;
export const TabsTrigger = ({ className, ...props }: TabsPrimitive.TabsTriggerProps) => <TabsPrimitive.Trigger className={cn("h-9 border-b-2 border-transparent px-3 text-xs font-medium text-muted outline-none hover:text-foreground focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-accent data-[state=active]:border-accent data-[state=active]:text-foreground", className)} {...props} />;
export const TabsContent = ({ className, ...props }: TabsPrimitive.TabsContentProps) => <TabsPrimitive.Content className={cn("min-h-0 flex-1 outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-accent", className)} {...props} />;
