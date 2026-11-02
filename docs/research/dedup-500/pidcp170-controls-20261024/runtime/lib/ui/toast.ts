// Explicit UI observer; no real toast or user messages.
export const notices: unknown[]=[];
export function toast(ctx: unknown,text: string,level: string){notices.push({text,level});}
