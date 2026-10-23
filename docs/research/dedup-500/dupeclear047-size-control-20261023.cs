using DupeClear.Converters;
using System.Globalization;
var converter=new BytesToKilobytesConverter();
object? Forward(long bytes)=>converter.Convert(bytes,typeof(decimal),null,CultureInfo.InvariantCulture);
long Back(decimal kb)=>(long)converter.ConvertBack(kb,typeof(long),null,CultureInfo.InvariantCulture)!;
void Check(bool condition,string label){if(!condition)throw new Exception(label);Console.WriteLine("PASS "+label);}
Check((decimal)Forward(2048)! ==2m && Back(2m)==2048,"2 KiB roundtrip");
Check((decimal)Forward(1536)! ==1m,"fractional KiB truncated before decimal cast");
Check(Back(1.5m)==2048,"fractional input rounded before scaling");
Check(Back((decimal)long.MaxValue)==-1024,"UI maximum wraps to negative byte limit in default unchecked build");
Check(Back((decimal)(long.MaxValue/1024))==9223372036854774784L,"largest whole KiB fitting Int64 remains positive");
Console.WriteLine("5 checks passed; original converter unchanged; interface-only Avalonia adapter, no UI runtime.");
