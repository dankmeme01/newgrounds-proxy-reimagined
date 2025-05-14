#include <Geode/Geode.hpp>
#include <Geode/loader/Dispatch.hpp>

using namespace geode::prelude;

static std::string g_proxyUrl;

void proxySend(CCHttpClient* self, CCHttpRequest* req) {
    constexpr std::string_view NG_DOMAIN = "audio.ngfiles.com";

    std::string_view url = req->getUrl();

    auto domainBegin = url.find(NG_DOMAIN);
    if (domainBegin == std::string::npos) {
        return self->send(req);
    }

    std::string newUrl = fmt::format("{}{}", g_proxyUrl, url.substr(domainBegin + NG_DOMAIN.size()));

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
