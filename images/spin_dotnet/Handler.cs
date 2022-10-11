using Fermyon.Spin.Sdk;

namespace MyTestProject;

public static class Handler
{
    [HttpHandler]
    public static HttpResponse HandleHttpRequest(HttpRequest request)
    {
        return new HttpResponse
        {
            StatusCode = System.Net.HttpStatusCode.OK,
            Headers = new Dictionary<string, string>
            {
                { "Content-Type", "text/plain" },
            },
            BodyAsString = "Hello from .NET\n",
        };
    }
}
