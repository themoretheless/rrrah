using Jellyfin.Plugin.MediaDash.Configuration;
using Jellyfin.Plugin.MediaDash.Scanners;
using MediaBrowser.Controller.Entities.Movies;
using Xunit;

namespace Jellyfin.Plugin.MediaDash.Tests;

// Finite controls against the complete original helper, not end-to-end scanner evidence.
public sealed class RrrahDuplicateScoringControls
{
    private static PluginConfiguration Policy() => new()
    {
        DuplicateTitleJaccardVeto = 0.40,
        DuplicateRuntimeVetoPct = 15
    };

    [Fact]
    public void ActualMovieRuntimeReachesTheScorePairVeto()
    {
        var a = new DuplicateScanner.Candidate { Path = "/a/Movie.mkv", Item = new Movie { RunTimeTicks = 6000 } };
        var b = new DuplicateScanner.Candidate { Path = "/b/Movie.mkv", Item = new Movie { RunTimeTicks = 10000 } };
        var result = DuplicateScanner.ScorePair(a, b, ConfidenceTier.Identified, false, false, Policy());
        Assert.True(result.Vetoed);
        Assert.Equal(0.0, result.Confidence);
        Assert.Equal("runtimeDeltaExceedsThreshold", result.Signals["vetoReason"]);
        Assert.Equal(0.40, (double)result.Signals["runtimeDelta"]!);
    }

    [Fact]
    public void DifferentReleaseGroupsCanVetoTheSameTitle()
    {
        var a = new DuplicateScanner.Candidate { Path = "/a/Inception.2010.1080p.BluRay.x264-YIFY.mkv" };
        var b = new DuplicateScanner.Candidate { Path = "/b/Inception.2010.2160p.WEB-DL.x265-RARBG.mkv" };
        var result = DuplicateScanner.ScorePair(a, b, ConfidenceTier.Identified, false, false, Policy());
        Assert.True(result.Vetoed);
        Assert.Equal("titleJaccardBelowThreshold", result.Signals["vetoReason"]);
        Assert.Equal(1.0 / 3.0, (double)result.Signals["titleJaccard"]!, precision: 6);
    }

    [Fact]
    public void CyrillicOnlyNamesLeaveTitleSignalUnavailable()
    {
        var a = new DuplicateScanner.Candidate { Path = "/a/Кошка.mkv" };
        var b = new DuplicateScanner.Candidate { Path = "/b/Собака.mkv" };
        var result = DuplicateScanner.ScorePair(a, b, ConfidenceTier.Identified, false, false, Policy());
        Assert.False(result.Vetoed);
        Assert.Null(result.Signals["titleJaccard"]);
        Assert.Equal(0.90, result.Confidence);
    }
}
