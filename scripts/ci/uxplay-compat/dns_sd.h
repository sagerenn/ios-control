#ifndef IOS_CONTROL_UXPLAY_COMPAT_DNS_SD_H
#define IOS_CONTROL_UXPLAY_COMPAT_DNS_SD_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#if defined(_WIN32) && !defined(EFI32) && !defined(EFI64)
#define DNSSD_STDCALL __stdcall
#else
#define DNSSD_STDCALL
#endif

typedef struct _DNSServiceRef_t *DNSServiceRef;
typedef union _TXTRecordRef_t {
    char PrivateData[16];
    char *ForceNaturalAlignment;
} TXTRecordRef;
typedef uint32_t DNSServiceFlags;
typedef int32_t DNSServiceErrorType;

typedef void(DNSSD_STDCALL *DNSServiceRegisterReply)(
    DNSServiceRef sdRef,
    DNSServiceFlags flags,
    DNSServiceErrorType errorCode,
    const char *name,
    const char *regtype,
    const char *domain,
    void *context
);

DNSServiceErrorType DNSSD_STDCALL DNSServiceRegister(
    DNSServiceRef *sdRef,
    DNSServiceFlags flags,
    uint32_t interfaceIndex,
    const char *name,
    const char *regtype,
    const char *domain,
    const char *host,
    uint16_t port,
    uint16_t txtLen,
    const void *txtRecord,
    DNSServiceRegisterReply callBack,
    void *context
);

void DNSSD_STDCALL DNSServiceRefDeallocate(DNSServiceRef sdRef);

void DNSSD_STDCALL TXTRecordCreate(
    TXTRecordRef *txtRecord,
    uint16_t bufferLen,
    void *buffer
);

void DNSSD_STDCALL TXTRecordDeallocate(TXTRecordRef *txtRecord);

DNSServiceErrorType DNSSD_STDCALL TXTRecordSetValue(
    TXTRecordRef *txtRecord,
    const char *key,
    uint8_t valueSize,
    const void *value
);

uint16_t DNSSD_STDCALL TXTRecordGetLength(const TXTRecordRef *txtRecord);

const void *DNSSD_STDCALL TXTRecordGetBytesPtr(const TXTRecordRef *txtRecord);

#ifdef __cplusplus
}
#endif

#endif
