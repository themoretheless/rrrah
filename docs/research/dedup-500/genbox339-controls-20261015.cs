using System;
using System.Collections.Generic;
using System.Linq;
using System.Reflection;
using System.Text;
using FindDupes;
using FindDupes.Hash;

public static class Probe
{
    private static int checks;
    private static void Check(bool value, string label)
    {
        if (!value) throw new Exception(label);
        checks++;
    }
    private static object Invoke(string name, params object[] arguments) =>
        typeof(FileDetails).Assembly.GetType("FindDupes.Program")
            .GetMethod(name, BindingFlags.Static | BindingFlags.NonPublic)
            .Invoke(null, arguments);
    private static List<List<FileDetails>> Groups(List<FileDetails> files)
    {
        var sizes = ((IEnumerable<List<FileDetails>>)Invoke("GroupBySize", files)).ToList();
        return ((IEnumerable<List<FileDetails>>)Invoke("GroupByCriteria", sizes)).ToList();
    }
    public static void Main()
    {
        var comparer = new FileDetailsComparer();
        var time = DateTimeOffset.FromUnixTimeMilliseconds(1);
        var a = new FileDetails("synthetic-a", 3, time, 77);
        var b = new FileDetails("synthetic-b", 3, time, 77);
        Check(comparer.Equals(a, b), "equal stored hash");
        Check(Groups(new List<FileDetails> { a, b }).Single().Count == 2,
            "injected same hash groups candidates without content confirmation");
        b.Hash = 78;
        Check(Groups(new List<FileDetails> { a, b }).Count == 0, "different hashes");
        Check(Groups(new List<FileDetails> { a, new FileDetails("c", 4, time, 77) }).Count == 0,
            "size grouping precedes comparer");
        Check(comparer.Equals(a, new FileDetails("c", 4, time, 77)), "comparer itself ignores size");
        Check(!comparer.Equals(a, new FileDetails("c", 3, time.AddSeconds(1), 77)), "timestamp filter");
        var missingA = new FileDetails("/private/tmp/rrrah-genbox339-controls-20261015/absent-a", 3, time);
        var missingB = new FileDetails("/private/tmp/rrrah-genbox339-controls-20261015/absent-b", 3, time);
        foreach (var missing in new[] { missingA, missingB })
        {
            object[] arguments = { missing, null };
            Check(!(bool)Invoke("TryReadFile", arguments) && arguments[1] == null,
                "missing file read returns false and null");
            Check(missing.Hash == 0, "failed read leaves default hash");
        }
        Check(Groups(new List<FileDetails> { missingA, missingB }).Single().Count == 2,
            "two unreadable same-size records group as duplicates");
        var inputs = new[] { "Hello World", "\0", "1" };
        ulong[] xx = { 10764455564493180894, 11145182160106660097, 15532033055997473560 };
        ulong[] sha = { 5750913225038017743, 3001610411640981780, 4397373564687294441 };
        for (int i = 0; i < inputs.Length; i++)
        {
            var bytes = Encoding.Unicode.GetBytes(inputs[i]);
            Check(new xxHash().GetHash(bytes) == xx[i], "upstream xxHash vector");
            Check(new SHA1Hash().GetHash(bytes) == sha[i], "upstream SHA1 vector");
        }
        Console.WriteLine($"CHECKS={checks} UNREADABLE_GROUP=2 INJECTED_HASH_GROUP=2");
    }
}

// CommandLineParser is compile-only: original CLI Main is never called.
namespace CommandLine
{
    public class OptionAttribute : Attribute
    {
        public OptionAttribute(string name) { }
        public OptionAttribute(char shortName, string name) { }
        public bool Required { get; set; }
        public string HelpText { get; set; }
        public object Default { get; set; }
    }
    public class ValueAttribute : Attribute
    {
        public ValueAttribute(int index) { }
        public string MetaName { get; set; }
        public bool Required { get; set; }
        public string HelpText { get; set; }
    }
    public class UsageAttribute : Attribute { }
    public class ParserResult<T> { }
    public class Parsed<T> : ParserResult<T> { public T Value { get; set; } }
    public class Parser
    {
        public static Parser Default => throw new Exception("CLI adapter must never run");
        public ParserResult<T> ParseArguments<T>(string[] args) => throw new Exception("CLI adapter must never run");
    }
}
namespace CommandLine.Text
{
    public class Example { public Example(string description, object options) { } }
}
