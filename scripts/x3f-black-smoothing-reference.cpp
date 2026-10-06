// Test-only float arithmetic reference for legacy Foveon black smoothing.
#include <algorithm>
#include <cstdint>
#include <cmath>
#include <cstring>
#include <fstream>
#include <vector>
int main(int argc,char **argv) {
    if(argc!=3 && argc!=4)return 2;
    std::ifstream in(argv[1],std::ios::binary);
    std::vector<float> b(1531*3);in.read(reinterpret_cast<char*>(b.data()),b.size()*4);
    if(!in)return 1;
    const int h=1531;
    std::copy(b.begin()+24,b.begin()+48,b.begin());
    std::copy(b.begin()+(h-22)*3,b.begin()+(h-11)*3,b.begin()+(h-11)*3);
    float last[3][3];std::memcpy(last,b.data(),sizeof(last));
    int r;
    for(r=1;r<h-1;++r) {
        for(int c=0;c<3;++c) {
            if(last[1][c]>last[0][c]) {
                if(last[1][c]>last[2][c])b[r*3+c]=std::max(last[0][c],last[2][c]);
            } else if(last[1][c]<last[2][c])b[r*3+c]=std::min(last[0][c],last[2][c]);
        }
        std::memmove(last,last+1,6*sizeof(float));
        std::memcpy(last[2],b.data()+(r+1)*3,3*sizeof(float));
    }
    for(int c=0;c<3;++c) {
        b[r*3+c]=(last[0][c]+last[1][c])/2;
        b[c]=(b[3+c]+b[9+c])/2;
    }
    float alpha=1-std::exp(-1/24.0),sum[3];std::memcpy(sum,b.data(),sizeof(sum));
    for(r=1;r<h;++r)for(int c=0;c<3;++c) {
        b[r*3+c]=(b[r*3+c]-b[(r-1)*3+c])*alpha+b[(r-1)*3+c];sum[c]+=b[r*3+c];
    }
    std::memcpy(last[0],b.data()+(h-1)*3,3*sizeof(float));
    for(int c=0;c<3;++c)sum[c]/=h;
    for(r=h-1;r>=0;--r)for(int c=0;c<3;++c)
        last[0][c]=b[r*3+c]=(b[r*3+c]-sum[c]-last[0][c])*alpha+last[0][c];
    if(argc==4) {
        std::ifstream sensor_file(argv[3],std::ios::binary);
        std::vector<int16_t> sensor(2304*1531*3);
        sensor_file.read(reinterpret_cast<char*>(sensor.data()),sensor.size()*2);
        if(!sensor_file)return 1;
        int64_t totals[3]={};int64_t count=0;
        for(int y=2;y<1531;y+=4)for(int x=2;x<2304;x+=4) {
            for(int c=0;c<3;++c)totals[c]+=sensor[(y*2304+x)*3+c];
            ++count;
        }
        for(int y=0;y<1531;++y)for(int c=0;c<3;++c)
            b[y*3+c]+=sum[c]/2+totals[c]/(count*100.0);
    }
    std::ofstream out(argv[2],std::ios::binary);out.write(reinterpret_cast<char*>(b.data()),b.size()*4);
    return out?0:1;
}
