// Real filesystem/original main.cpp control flow; explicit image/hash/log adapters.
// No actual OpenCV/pHash/SisIO binary, image quality or whole CLI qualification.
#include <algorithm>
#include <filesystem>
#include <fstream>
#include <iostream>
#include <functional>
#include <map>
#include <vector>
#include <stdexcept>
static std::vector<std::string> output_records;
static std::map<std::string,unsigned long long> fingerprints;
static std::string last_source;
static bool hash_failure=false;
static int decodes=0,encodes=0;
#define main original_main
#include "original-main.cpp"
#undef main
SisIO::SisIO(std::string name):logFilePath(name),logLevel(messageType::info),outputLevel(messageType::info),colorMessages(false),tagMessages(false),colorReset(""){}
SisIO::~SisIO(){}
int SisIO::output(messageType,const std::string&,bool){return 0;}
int SisIO::log(messageType,const std::string& message,const std::string){if(logFilePath==OUTPUT_FILE)output_records.push_back(message);return 0;}
std::string SisIO::inputLine(const std::string&){return "";}
cv::Mat cv::imread(const std::string& path,int){++decodes;last_source=path;return {path,!std::filesystem::exists(path)};}
bool cv::imwrite(const std::string& path,const cv::Mat&,const std::vector<int>&){++encodes;std::ofstream f(path,std::ios::binary);f<<"synthetic-adapter-jpeg";return bool(f);}
int ph_dct_imagehash(const char *,ulong64& value){value=fingerprints[std::filesystem::path(last_source).filename().string()];return hash_failure?-1:0;}
int ph_hamming_distance(ulong64 a,ulong64 b){return __builtin_popcountll(a^b);}
static void require(bool value,const char* message){if(!value)throw std::runtime_error(message);}
static void touch(const std::filesystem::path& path,const char* data="owned synthetic bytes"){std::ofstream f(path);f<<data;require(bool(f),"write owned fixture");}
int main(int argc,char**argv){
 require(argc==2,"owned fixture root required");std::filesystem::path root=argv[1];require(std::filesystem::is_directory(root),"owned root exists");const auto old=std::filesystem::current_path();std::filesystem::current_path(root);
 int passed=0;
 auto run=[&](const char*name,const std::function<void(std::filesystem::path)>& test){auto dir=root/std::to_string(passed);std::filesystem::create_directory(dir);output_records.clear();fingerprints.clear();hash_failure=false;decodes=encodes=0;threshold=15;test(dir);++passed;std::cout<<"PASS "<<name<<'\n';};
 try{
  run("extension_case_and_format_admission",[&](auto dir){touch(dir/"a.JPG");touch(dir/"a.webp");touch(dir/"a.gif");require(!isImageFile(dir/"a.JPG"),"uppercase omitted");require(isImageFile(dir/"a.webp"),"webp admitted");require(!isImageFile(dir/"a.gif"),"gif omitted");});
  run("jpeg_extension_dot_mismatch_forces_conversion",[&](auto dir){touch(dir/"a.jpg");require(convert(dir/"a.jpg"),"adapter converts");require(decodes==1&&encodes==1,"jpg passed through decode and encode");cleanup();});
  run("equal_hash_overwrite_loses_one_of_three_pairs",[&](auto dir){for(auto name:{"a.jpg","b.jpg","c.jpg"}){touch(dir/name);fingerprints[name]=7;}std::unordered_map<ulong64,std::filesystem::path> map;compute(dir,map);require(map.size()==1&&output_records.size()==2,"three equal signatures produce only2pairs");cleanup();});
  run("threshold_is_exclusive",[&](auto dir){touch(dir/"a.jpg");touch(dir/"b.jpg");fingerprints["a.jpg"]=0;fingerprints["b.jpg"]=(1ULL<<15)-1;std::unordered_map<ulong64,std::filesystem::path> map;compute(dir,map);require(output_records.empty(),"distance15 excluded at15");threshold=16;map.clear();compute(dir,map);require(output_records.size()==1,"same distance accepted at16");cleanup();});
  run("hash_provider_failure_with_initialized_value_is_ignored",[&](auto dir){touch(dir/"a.jpg");touch(dir/"b.jpg");fingerprints["a.jpg"]=fingerprints["b.jpg"]=17;hash_failure=true;std::unordered_map<ulong64,std::filesystem::path> map;compute(dir,map);require(map.size()==1&&output_records.size()==1,"failure status ignored and candidate published");cleanup();});
  run("fixed_temporary_path_overwrites_existing_owned_file",[&](auto dir){touch(dir/"a.png");touch(TEMP_FILE,"owned sentinel");require(convert(dir/"a.png"),"adapter conversion");std::ifstream f(TEMP_FILE);std::string value;std::getline(f,value);require(value=="synthetic-adapter-jpeg","existing temp overwritten");cleanup();});
  run("cleanup_removes_existing_owned_file_without_creation_proof",[&](auto){touch(TEMP_FILE,"owned sentinel");cleanup();require(!std::filesystem::exists(TEMP_FILE),"preexisting temp removed");});
  run("threshold_overflow_escapes_invalid_argument_handler",[&](auto dir){std::string program="DupIm",target=dir.string(),number(200,'9');char* args[]={program.data(),target.data(),number.data(),nullptr};bool threw=false;try{original_main(3,args);}catch(const std::out_of_range&){threw=true;}require(threw,"stoi overflow unhandled by original main");});
  run("negative_threshold_not_validated",[&](auto dir){std::string program="DupIm",target=dir.string(),number="-1";char*args[]={program.data(),target.data(),number.data(),nullptr};require(original_main(3,args)==0&&threshold==-1,"negative threshold accepted");});
  std::cout<<"TOTAL "<<passed<<" PASS\n";
 }catch(...){std::filesystem::current_path(old);throw;}
 std::filesystem::current_path(old);return 0;
}
