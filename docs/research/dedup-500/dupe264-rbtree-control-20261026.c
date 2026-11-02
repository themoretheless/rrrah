#include <assert.h>
#include <stdio.h>
#include "rbtree_augmented.h"
struct item {int key;struct rb_node node;};
static int check(struct rb_node*n,struct rb_node*p,int lo,int hi,int*count){
 if(!n)return 1;
 struct item*x=rb_entry(n,struct item,node); assert(x->key>lo&&x->key<hi);assert(rb_parent(n)==p);
 if(rb_is_red(n)){assert(!n->rb_left||rb_is_black(n->rb_left));assert(!n->rb_right||rb_is_black(n->rb_right));}
 ++*count;int a=check(n->rb_left,n,lo,x->key,count),b=check(n->rb_right,n,x->key,hi,count);assert(a==b);return a+rb_is_black(n);
}
static void verify(struct rb_root*r,int expected){int count=0;if(r->rb_node)assert(rb_is_black(r->rb_node));check(r->rb_node,NULL,-1,258,&count);assert(count==expected);int prev=-1,n=0;for(struct rb_node*p=rb_first(r);p;p=rb_next(p)){int k=rb_entry(p,struct item,node)->key;assert(k>prev);prev=k;++n;}assert(n==expected);prev=258;n=0;for(struct rb_node*p=rb_last(r);p;p=rb_prev(p)){int k=rb_entry(p,struct item,node)->key;assert(k<prev);prev=k;++n;}assert(n==expected);}
int main(void){struct rb_root r=RB_ROOT;struct item items[257],replacement;verify(&r,0);
 for(int i=0;i<257;i++){int k=(i*73)%257;items[k].key=k;struct rb_node**link=&r.rb_node,*parent=NULL;while(*link){parent=*link;link=k<rb_entry(parent,struct item,node)->key?&parent->rb_left:&parent->rb_right;}rb_link_node(&items[k].node,parent,link);rb_insert_color(&items[k].node,&r);verify(&r,i+1);}
 replacement.key=128;rb_replace_node(&items[128].node,&replacement.node,&r);verify(&r,257);
 for(int i=0;i<257;i++){int k=(i*89)%257;rb_erase(k==128?&replacement.node:&items[k].node,&r);verify(&r,256-i);}
 puts("PASS: 257 inserts, same-key replacement, 257 erases; order, parents, red rule and black height checked after every operation");return 0;}
