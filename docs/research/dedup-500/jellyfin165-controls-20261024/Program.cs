using System.Text;
using System.Text.Json;
using System.Text.Json.Nodes;
using Jellyfin.Plugin.ContinueWatchingDedup;
using Jellyfin.Plugin.ContinueWatchingDedup.Configuration;
using Jellyfin.Plugin.ContinueWatchingDedup.Middleware;
using Microsoft.AspNetCore.Http;
using Microsoft.Extensions.Logging.Abstractions;

var observations = new List<object>();
async Task<string> Invoke(string payload, string path="/UserItems/Resume", PluginConfiguration? config=null, bool fail=false)
{
    Plugin.Instance.Configuration = config ?? new();
    var ctx = new DefaultHttpContext();
    ctx.Request.Path = path;
    using var output = new MemoryStream();
    ctx.Response.Body = output;
    var middleware = new DedupMiddleware(async c => {
        await c.Response.Body.WriteAsync(Encoding.UTF8.GetBytes(payload));
        if (fail) throw new InvalidOperationException("owned downstream failure");
    }, NullLogger<DedupMiddleware>.Instance);
    await middleware.InvokeAsync(ctx);
    if (!ReferenceEquals(ctx.Response.Body,output)) throw new Exception("body not restored");
    return Encoding.UTF8.GetString(output.ToArray());
}
void Check(string name, bool passed, object evidence)
{
    observations.Add(new {name,passed,evidence});
    if (!passed) throw new Exception(name);
}
string episodes = """{"Items":[{"Id":"old","Type":"Episode","SeriesId":"s","UserData":{"LastPlayedDate":"2026-01-01T00:00:00Z"}},{"Id":"movie","Type":"Movie"},{"Id":"new","Type":"Episode","SeriesId":"s","UserData":{"LastPlayedDate":"2026-02-01T00:00:00Z"}}],"TotalRecordCount":99}""";
var filtered = JsonNode.Parse(await Invoke(episodes))!;
var ids = filtered["Items"]!.AsArray().Select(x=>x!["Id"]!.GetValue<string>()).ToArray();
Check("latest episode retained; surviving input order preserved", ids.SequenceEqual(new[]{"movie","new"}) && filtered["TotalRecordCount"]!.GetValue<int>()==2, ids);
Check("unrelated path passthrough", await Invoke(episodes,"/owned/other")==episodes, "byte-for-byte JSON passthrough");
const string malformed="{owned invalid JSON";
Check("malformed JSON fallback preserves payload",await Invoke(malformed)==malformed,"original bytes returned");
string failureResult=await Invoke("owned partial downstream body",fail:true);
Check("downstream exception swallowed and buffered bytes lost", failureResult.Length==0, new {outputLength=failureResult.Length,exceptionPropagated=false});
const string movies="""{"Items":[{"Id":"same","Type":"Movie"},{"Id":"same","Type":"Movie"}]}""";
var movieResult=JsonNode.Parse(await Invoke(movies,config:new(){DeduplicateMovies=true,MaxEpisodesPerSeries=2}))!;
Check("episode limit also permits duplicate movie records", movieResult["Items"]!.AsArray().Count==2, new {retained=2});
Console.WriteLine(JsonSerializer.Serialize(new {scope="Original complete middleware and configuration; real ASP.NET DefaultHttpContext and .NET streams; two Jellyfin type seams; no Jellyfin server/media accessed",authoredControls=observations},new JsonSerializerOptions{WriteIndented=true}));
