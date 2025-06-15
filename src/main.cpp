#include <Geode/Geode.hpp>
#include <Geode/loader/Dispatch.hpp>

using namespace geode::prelude;

static std::string g_proxyUrl;

void proxySend(CCHttpClient* self, CCHttpRequest* req) {
    std::vector<std::string_view> NG_DOMAINS = {
        "audio.ngfiles.com",
        "ngfiles-proxy.b-cdn.net",
    };

    std::string_view url = req->getUrl();

    bool found = false;
    std::string_view::iterator domainBegin = url.begin();

    for (const auto& NG_DOMAIN : NG_DOMAINS) {
        auto it = std::search(
            url.begin(), url.end(),
            NG_DOMAIN.begin(), NG_DOMAIN.end()
        );

        if (it != url.end()) {
            found = true;
            domainBegin = it;
            break;
        }
    }

    if (!found) {
        return self->send(req);
    }

    // find end of domain
    auto pathStart = std::find(domainBegin, url.end(), '/');

    if (pathStart == url.end()) {
        // no path, just return
        return self->send(req);
    }

    std::string newUrl = g_proxyUrl + std::string(pathStart, url.end());

    log::debug("Redirecting {} to {}", url, newUrl);

    req->setUrl(newUrl.c_str());
    self->send(req);
}

$execute {
    g_proxyUrl = Mod::get()->getSettingValue<std::string>("url");
    if (g_proxyUrl.empty()) {
        g_proxyUrl = "https://ngproxy.dankmeme.dev";
    }

    listenForSettingChanges("url", [](std::string url) {
        g_proxyUrl = url;
        if (g_proxyUrl.empty()) {
            g_proxyUrl = "https://ngproxy.dankmeme.dev";
        }
    });

    (void) Mod::get()->hook(
        reinterpret_cast<void*>(
			geode::addresser::getNonVirtual(&cocos2d::extension::CCHttpClient::send)
        ),
        &proxySend,
        "cocos2d::extension::CCHttpClient::send"
    );
}
