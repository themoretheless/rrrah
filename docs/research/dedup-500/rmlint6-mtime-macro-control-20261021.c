#include <math.h>
#include <stdio.h>
#define FLOAT_SIGN_DIFF(X, Y, tol) ((X) - (Y) > (tol) ? 1 : ((Y) - (X) > (tol) ? -1 : 0))
int main(void){double observed=1700000000.0; int same=FLOAT_SIGN_DIFF(observed,observed,0.000001);int changed=FLOAT_SIGN_DIFF(observed,observed+1,0.000001);int invalid=FLOAT_SIGN_DIFF(NAN,observed,0.000001);printf("same=%d changed=%d nan=%d\n",same,changed,invalid);return !(same==0&&changed==-1&&invalid==0);}
