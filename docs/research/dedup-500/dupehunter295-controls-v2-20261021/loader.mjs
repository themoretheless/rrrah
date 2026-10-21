import {registerHooks} from 'node:module';
import {pathToFileURL} from 'node:url';
import path from 'node:path';
const root=pathToFileURL(path.resolve('.dedup-work/dupehunter295-20261021')+'/').href;
registerHooks({resolve(specifier,context,next){
 if(context.parentURL?.startsWith(root)&&specifier.startsWith('.')&&specifier.endsWith('.js'))return next(specifier.slice(0,-3)+'.ts',context);
 return next(specifier,context);
}});
