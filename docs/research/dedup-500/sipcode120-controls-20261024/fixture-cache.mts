export const sanitizeSessionId=s=>s;
export const sessionCachePath=(h,s)=>h+'/'+s;
export const realStoreIO={};
export const loadReadCache=(p,io)=>io.loadCache(p);
export const appendReadEntry=(p,e,io)=>io.appendEntry(p,e);
