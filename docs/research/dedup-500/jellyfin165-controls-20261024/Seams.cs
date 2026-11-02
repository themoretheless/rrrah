// Only the Jellyfin configuration base and plugin singleton are represented.
// Middleware, JSON, streams, compression and HTTP context use original/.NET code.
namespace MediaBrowser.Model.Plugins { public class BasePluginConfiguration {} }
namespace Jellyfin.Plugin.ContinueWatchingDedup
{
    public class Plugin
    {
        public static Plugin Instance { get; } = new();
        public Configuration.PluginConfiguration Configuration { get; set; } = new();
    }
}
