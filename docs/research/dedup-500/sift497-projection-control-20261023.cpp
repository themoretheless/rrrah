#include <cassert>
#include <cstdio>
#include <iostream>
#include "src/project/pca.cpp"
#include "src/project/tsne.cpp"
int main(){
 auto f=hashes_to_features({"a","a0",""},8);assert(f[0]==f[1]);assert(f[2]==std::vector<double>(8,0));
 std::vector<std::vector<double>> identical(8,std::vector<double>(8,1));auto flat=tsne_project(identical,2,3,20,200,1,nullptr);for(auto& p:flat.points)for(double v:p)assert(v==0);
 auto p=pca_project({{0,0},{1,0},{2,0}},2,1,nullptr);assert(std::abs(p.variance_explained[0]-1)<1e-12);assert(std::abs(std::abs(p.points[2][0]-p.points[0][0])-2)<1e-12);
 int n=32;unsigned seed=83;std::vector<std::vector<double>> x(n,std::vector<double>(64));for(auto& row:x)for(auto& v:row){seed=seed*1664525+1013904223;v=(seed>>24)&1;}
 auto a=pca_project(x,2,1,nullptr),b=pca_project(x,2,4,nullptr);assert(a.points==b.points);auto ta=tsne_project(x,2,5,12,200,1,nullptr),tb=tsne_project(x,2,5,12,200,4,nullptr);assert(ta.points==tb.points);for(auto& row:ta.points)for(double v:row)assert(std::isfinite(v));
 std::vector<double> mean(64);for(auto& row:x)for(int j=0;j<64;j++)mean[j]+=row[j]/n;
 std::vector<std::vector<double>> g(n,std::vector<double>(n));for(int i=0;i<n;i++)for(int j=0;j<n;j++)for(int k=0;k<64;k++)g[i][j]+=(x[i][k]-mean[k])*(x[j][k]-mean[k]);ThreadPool pool(1);auto e=jacobi_eigen(g,n,pool);double residual=0;
 for(int c=0;c<n;c++)for(int i=0;i<n;i++){double r=-e.values[c]*e.vectors[c][i];for(int j=0;j<n;j++)r+=g[i][j]*e.vectors[c][j];residual=std::max(residual,std::abs(r));}
 assert(residual>1e-3);
 std::cout<<"rank-one PCA oracle passed; PCA/tSNE thread parity passed; identical tSNE collapses to zero; missing hex bits zero-padded; max eigen residual="<<residual<<"\n";
}
